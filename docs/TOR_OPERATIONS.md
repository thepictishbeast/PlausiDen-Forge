# Tor Operations — server-side runbook (#118)

**Status:** ops-side companion to the build-side
[`TOR_I2P_LOKINET_TEMPLATE.md`](TOR_I2P_LOKINET_TEMPLATE.md) and
[`external/how-to/deploy-to-tor.md`](external/how-to/deploy-to-tor.md).
This document covers the **Tor daemon + Caddy + systemd** side of a
hidden-service deployment — everything that lives on the box that
serves the `.onion`.

Closes task #224 / preamble #118.

---

## What this runbook covers

1. **`torrc` config** — Hidden Service v3 with current hardening directives
2. **Caddy config** — clearnet+onion mirror, `Onion-Location` header, no-JS-friendly cache headers
3. **systemd unit hardening** — minimum surface drop-ins for the `tor` service
4. **Key backup** — what files to preserve, how to restore on host migration
5. **Vanguards** — guard-discovery hardening addon for high-risk sites
6. **Monitoring** — descriptor-publish health + intro circuit observability
7. **Migration playbook** — moving a `.onion` to a new host without losing the address

## 1. `torrc` — Hidden Service v3 with hardening

```
# /etc/tor/torrc — minimal HSv3 service block

HiddenServiceDir /var/lib/tor/plausiden/
HiddenServicePort 80 127.0.0.1:8080
HiddenServiceVersion 3

# Cap concurrent streams per circuit. 0 = unlimited (default).
# Set 100-500 for a small site to refuse runaway connections.
HiddenServiceMaxStreams 500

# When a client tries to exceed the stream cap, close the circuit
# instead of just refusing the new stream (Tor 0.3.2+).
HiddenServiceMaxStreamsCloseCircuit 1

# Intro-circuit DoS defenses (Tor 0.4.6+). Set rate + burst per
# circuit to throttle attempted-introduce floods. Defaults are
# conservative; bump rate for genuinely-busy services.
HiddenServiceEnableIntroDoSDefense 1
HiddenServiceEnableIntroDoSRatePerSec 25
HiddenServiceEnableIntroDoSBurstPerSec 200

# Proof-of-work defenses (Tor 0.4.8+). For services that
# regularly face introduce-floods, enable PoW so clients have to
# burn CPU before the service allocates a rendezvous circuit.
# Default is off because it raises latency for legit clients.
# Turn on when under attack, off when not.
# HiddenServicePoWDefensesEnabled 1
# HiddenServicePoWQueueRate 250
# HiddenServicePoWQueueBurst 2500

# Client authorization (optional). Requires the visitor to
# present a curve25519 key to even resolve the descriptor.
# For genuinely-private services, not for public ones.
# ClientOnionAuthDir /var/lib/tor/onion-auth/

# Don't include this in advertised intro points — the descriptor
# includes intro-point info that, by itself, leaks no location
# but combined with a guard-discovery attack can be useful.
# Leave commented for public services; enable for high-risk.
# HiddenServiceSingleHopMode 0
# HiddenServiceNonAnonymousMode 0
```

**Do not set** `HiddenServiceSingleHopMode 1` or
`HiddenServiceNonAnonymousMode 1` unless you have explicitly
chosen to give up location anonymity for performance (e.g. a
high-traffic clearnet site that exposes a `.onion` purely for
client privacy, not publisher privacy). PlausiDen sites default
to full anonymity on both sides.

## 2. Caddy — clearnet + onion mirror

Caddy serves the same `static/` bytes on both origins. The
clearnet origin emits an `Onion-Location` header so Tor Browser
auto-redirects.

```
# /etc/caddy/Caddyfile (or a snippet imported into the main one)

example.com {
    root * /var/www/example
    file_server

    # Tor-Browser-friendly: declares the onion mirror. Browsers
    # that support it (Tor Browser 9.5+, Brave) auto-redirect.
    header Onion-Location http://EXAMPLEONIONv3onionaddresssXYZ.onion{uri}

    # Standard hardening on the clearnet side (HSTS, no referrer
    # leakage to upstreams, frame-ancestors).
    header Strict-Transport-Security "max-age=31536000; includeSubDomains; preload"
    header Referrer-Policy "no-referrer"
    header X-Content-Type-Options "nosniff"
    header Permissions-Policy "interest-cohort=()"
}

# .onion virtual host — same root, no TLS (Tor circuit is the
# transport encryption). Caddy listens on the plain HTTP port
# the torrc HiddenServicePort points at.
http://:8080 {
    bind 127.0.0.1
    root * /var/www/example
    file_server

    # Don't emit Onion-Location on the onion side — it'd create
    # a loop.

    # Don't emit Strict-Transport-Security on the onion side
    # either — there's no HTTPS to upgrade to.

    header Referrer-Policy "no-referrer"
    header X-Content-Type-Options "nosniff"
    header Permissions-Policy "interest-cohort=()"

    # Cache more aggressively on the onion side: every byte
    # over a Tor circuit is expensive.
    @static {
        path *.css *.js *.woff2 *.avif *.webp *.svg *.png *.jpg
    }
    header @static Cache-Control "public, max-age=31536000, immutable"
}
```

When the operator runs `forge deploy push tor-main` (per the
existing `external/how-to/deploy-to-tor.md`), the built bundle
ends up in `/var/www/example/`. Both vhosts serve from the same
directory — there is exactly one bundle.

## 3. systemd unit hardening for `tor`

Debian/Ubuntu's `tor.service` is reasonably hardened out of the
box. The drop-in below adds the **incremental** directives that
are still missing and have been verified safe for tor's runtime
needs.

Per memory [[systemd-hardening-incremental]], apply one at a time
+ test with `systemd-analyze security tor.service`.

```
# /etc/systemd/system/tor.service.d/hardening.conf

[Service]
# tor needs to read /etc/tor + write /var/lib/tor + bind sockets.
ProtectSystem=strict
ReadWritePaths=/var/lib/tor /var/log/tor
ProtectHome=true
PrivateTmp=true
PrivateDevices=true
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6
RestrictRealtime=true
RestrictNamespaces=true
LockPersonality=true
MemoryDenyWriteExecute=true
NoNewPrivileges=true

# Keep these LAST in your incremental rollout — they have the
# highest chance of breaking a working tor:
# SystemCallFilter=@system-service
# SystemCallErrorNumber=EPERM
```

Reload + verify:

```bash
sudo systemctl daemon-reload
sudo systemctl restart tor
sudo systemd-analyze security tor.service
```

The score should land in the "OK" or "exposed" band (1.0-4.0).
"Unprotected" (>9.0) means the drop-in didn't apply.

## 4. Key backup — what to preserve, how to restore

The hidden service identity lives in three files under
`HiddenServiceDir`:

```
/var/lib/tor/plausiden/
├── authorized_clients/    # client-auth keys (optional)
├── hostname               # PUBLIC: the .onion address
├── hs_ed25519_public_key  # PUBLIC: the ed25519 public key
└── hs_ed25519_secret_key  # SECRET: lose this = lose the address
```

The `.onion` hostname is derived from `hs_ed25519_public_key`.
If `hs_ed25519_secret_key` is destroyed, **the address is gone
forever** — by design, that's the property that makes the
hostname location-independent + publisher-controlled.

**Backup recipe** (run from a privileged shell):

```bash
sudo tar --numeric-owner -czf /root/tor-onion-keys-$(date +%Y%m%dT%H%M%SZ).tar.gz \
  -C /var/lib/tor plausiden/
```

Store the archive on **separate cold media** (encrypted USB, a
hardware-key-encrypted file on a different machine, etc.). Do
NOT push it to git. Do NOT email it. The secret key is enough
to impersonate the service for as long as it's not rotated.

**Restore on a new host:**

```bash
sudo systemctl stop tor
sudo mkdir -p /var/lib/tor/plausiden
sudo tar --numeric-owner -xzf /path/to/tor-onion-keys.tar.gz -C /var/lib/tor
sudo chown -R debian-tor:debian-tor /var/lib/tor/plausiden
sudo chmod 700 /var/lib/tor/plausiden
sudo systemctl start tor
sudo tail -f /var/log/tor/log
```

After a few seconds you should see `[notice] HiddenService: …
descriptor published for service …` and the `.onion` resolves
from any Tor client.

## 5. Vanguards — guard-discovery defense

For sites that face a determined adversary (state-actor-level),
the default 3-hop Tor circuit is vulnerable to *guard discovery*
— an attacker who can observe enough circuits over time can
narrow down which guard relay the hidden service is using, and
from there work backward to the host.

Mitigation: the **vanguards addon** (https://github.com/mikeperry-tor/vanguards),
which pins the second-layer guards to a small rotating set so an
attacker can't trivially observe a fresh second-hop on every
attempt.

```bash
# Install as root
sudo apt install python3-stem
git clone https://github.com/mikeperry-tor/vanguards /opt/vanguards
sudo cp /opt/vanguards/vanguards-example.conf /etc/tor/vanguards.conf
# Edit /etc/tor/vanguards.conf — set control_socket + control_pass.

# Systemd unit
sudo cp /opt/vanguards/contrib/vanguards.service /etc/systemd/system/
sudo systemctl enable --now vanguards
sudo systemctl status vanguards
```

For public/lower-risk sites the **vanguards-lite** that ships in
Tor 0.4.7+ (built-in, no addon needed) is enough. It activates
automatically for hidden services.

## 6. Monitoring

What to watch:

* **Descriptor publish** — `/var/log/tor/log` should show
  `HiddenService: descriptor published for service <addr>`
  within ~60s of `systemctl restart tor`. Subsequent re-publishes
  happen every ~60-90 minutes.
* **Intro-circuit health** — `tor` doesn't expose intro-point
  state in `/var/log/tor/log` by default; raise verbosity with
  `Log [hs]info file /var/log/tor/hs-info.log` if needed for
  diagnostics, then turn it back off (intro-point list is
  sensitive).
* **Caddy 5xx on the onion side** — `/var/log/caddy/access.log`
  filtered for the bound `127.0.0.1:8080` virtual host.
* **Filesystem hardening intact** — `systemd-analyze security
  tor.service` should keep its low score after every dist-upgrade
  (Debian ships occasional `tor.service` updates that can revert
  drop-ins). Add to the weekly host audit.

Simple journalctl check (run on hourly cron + alert on absence):

```bash
journalctl --since "2 hours ago" -u tor | grep -c "descriptor published" 
```

Should be ≥ 1. If it returns 0, the descriptor failed to
publish — the service is effectively unreachable until the next
publish attempt.

## 7. Migration playbook

To move an existing `.onion` to a new host without losing the
address:

1. **Source host**: backup keys per §4. Stop tor: `sudo
   systemctl stop tor`. Tar up `/var/www/example/` for content.
2. **Target host**: install tor + caddy. Restore keys per §4.
   Copy `/var/www/example/` contents. Set up `torrc` + Caddyfile
   per §1 + §2.
3. **Target host**: `sudo systemctl start tor`. Wait for
   `descriptor published` in the log.
4. **Verify**: from any Tor client, fetch the `.onion` URL.
   Should serve the same bytes as the source host did.
5. **Source host**: ONLY THEN drop DNS / shut down. If you stop
   the source before the target has published, there's a window
   where the descriptor in the HSDir cache still points at the
   old intro circuits and the service is unreachable.
6. **Verify multiple-DA reach**: Tor's HSDirs cache descriptors
   for ~3h; restart-then-reach roundtrips can take that long
   under adversarial network conditions. Don't panic-rotate if
   the first 60s shows "unreachable."

DO NOT run the same hidden service simultaneously on two hosts.
Tor's HSDir consensus will see conflicting descriptors and the
service becomes intermittently unreachable. Stop the source
fully before starting the target's identity (you can rsync the
content while source is up, but flip the keys atomically).

## What this runbook intentionally does NOT cover

* **The Forge build side** — see `TOR_I2P_LOKINET_TEMPLATE.md`.
* **The forge-cli deploy adapter** — see
  `external/how-to/deploy-to-tor.md`.
* **I2P / Lokinet operations** — separate transports, separate
  runbooks. The Forge build side handles all three identically
  (per `network_target_enforcement`), but the daemon configs
  differ; this doc is Tor-specific.
* **Bridging to Tor (obfs4, snowflake, etc.)** — only relevant
  for *clients* in censored networks reaching this service. Not
  for the publisher side.
* **Bundled in-host Tor relay** — running a relay AND a hidden
  service on the same host is supported but has its own risks
  (correlation between relay traffic + service usage); for most
  PlausiDen deployments, keep them separate.

## Future work

* **Onion-Location header automation** — `forge deploy push`
  should emit a Caddy snippet automatically when both `clearnet`
  and `tor` targets are configured + the `.onion` is known. Today
  it's a manual edit.
* **systemd-creds-based key storage** — store the
  `hs_ed25519_secret_key` in `systemd-creds` rather than the
  filesystem, with `LoadCredential=hs-key:/var/lib/tor/plausiden/hs_ed25519_secret_key`
  + a tmpfs binding. Closer to AVP-3 hardening posture; not
  shipped yet because tor needs the key file at a predictable
  path during startup and the credential overlay is fiddly.
* **Per-tenant onion isolation** — each tenant getting their own
  HiddenServiceDir, served from a per-tenant `/var/www` root.
  Today multi-tenant deploys share one `.onion`; this is fine
  for sites with one operator but doesn't scale to PlausiDen-
  Loom's SaaS-style multi-tenant model.
