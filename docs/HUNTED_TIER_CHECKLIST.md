# Hunted-tier Build Checklist (#123)

**Status:** operator-facing pre-publish checklist + threat model
+ verification recipe. Companion to the existing
[`TOR_I2P_LOKINET_TEMPLATE.md`](TOR_I2P_LOKINET_TEMPLATE.md),
[`TOR_OPERATIONS.md`](TOR_OPERATIONS.md), and Loom's
[`NOSCRIPT_AUDIT.md`](../../PlausiDen-Loom/docs/NOSCRIPT_AUDIT.md).

Closes task #229 / preamble #123.

The `hunted_tier` Forge phase already exists at code level
(`crates/forge-phases/src/hunted_tier.rs`). This document is the
**operator's pre-publish checklist** — what to verify before
declaring a build "hunted-tier" and pointing real users at it.

---

## What "hunted-tier" defends against

The threat model is **a determined adversary with state-actor-
level resources who can subpoena services, run traffic analysis,
and surveil third-party CDNs**. Specifically:

* **TLS interception** — adversary can MITM clearnet traffic at
  ISP or country level.
* **CDN-level surveillance** — Cloudflare, Cloudfront, Akamai,
  Google Fonts, jsDelivr, etc. all see request metadata. Any
  resource served from these origins logs the visitor.
* **Browser-level fingerprinting** — canvas, WebGL, AudioContext,
  Battery API, fonts, plugins — all yield bits that uniquely
  identify a browser session.
* **Server-side honeypot** — even a co-operating server can't
  log what isn't sent. Hunted-tier minimizes what arrives at
  the box.
* **Subpoena pressure** — the operator may be compelled to
  produce logs. Hunted-tier ensures the logs are uninteresting
  by ensuring the visitor traffic carries minimal identifiers.

What hunted-tier does NOT address (out of band):

* The visitor's threat model — Tor Browser at "Safest" is the
  assumed client. A site can be hunted-tier and the visitor
  can still de-anonymize themselves via OPSEC failures unrelated
  to the build.
* Server compromise — if the box is rooted, hunted-tier on
  the deploy doesn't matter. That's a TLS/SSH/auth problem.
* Tor exit-node correlation for clearnet traffic — only the
  `.onion` mirror gives publisher-side anonymity. Clearnet at
  CDN-free is harder than the `.onion`, by design.
* Network-level timing attacks — Vanguards / Vanguards-Lite
  partially mitigate; full protection requires bridges / pluggable
  transports on the client side, which is out of band.

## Configuration

Minimum `forge.toml` for a hunted-tier build:

```toml
[security]
tier = "hunted"

[noscript_strict]
enabled = true

[reader_safety]
strict = true

[networks]
targets = ["tor"]
# Or for clearnet+tor co-publish:
# targets = ["clearnet", "tor"]
# tor_address = "abc...xyz.onion"   # if mirroring
```

Build invocation:

```bash
LOOM_NOSCRIPT_MODE=1 forge build
```

The env flag tells Loom's `page_shell_themed` to drop every
inline `<script>` + the defer-stylesheet onload swap, emit the
strictest CSP (`script-src 'none'`, `require-trusted-types-for
'script'`, `trusted-types 'none'`), and use the CSS-only
`:has()`-driven theme toggle (commit 7b82814).

## What the `hunted_tier` Forge phase enforces

From `crates/forge-phases/src/hunted_tier.rs`:

1. **`noscript_strict` MUST be enabled** — either via
   `forge.toml [noscript_strict] enabled = true` OR via
   `LOOM_NOSCRIPT_MODE=1` in the build env. The phase fails
   strict if neither is set.
2. **Body-text leak scan** — rendered HTML in `static/*.html`
   is scanned for any of these literal markers (full
   `BODY_LEAK_MARKERS` list, kept in sync with the const):

   * `localStorage.`
   * `sessionStorage.`
   * `document.cookie`
   * `navigator.geolocation`
   * `navigator.mediaDevices`
   * `navigator.usb`
   * `navigator.bluetooth`
   * `navigator.serial`
   * `canvas.toDataURL`
   * `getContext('webgl')`
   * `WebGLRenderingContext`
   * `navigator.getBattery`

   Each hit is a strict finding. If a marker is innocent body
   copy (article *about* localStorage, etc.), opt out of the
   hunted tier for that build with `tier = "strict"`.
3. **`Set-Cookie` literal scan** — the case-insensitive string
   "set-cookie" anywhere in rendered HTML fires strict. A
   site that declares Set-Cookie in HTML implies server-state
   even if the JS that uses it got stripped.

Adjacent Forge phases that the hunted tier composes (each runs
independently; hunted_tier is a shape-check that they're on):

* `noscript_strict` — confirms zero `<script>` tags in `static/`.
* `network_target_enforcement` — refuses any clearnet host
  reference (script src, link href, img src, fetch URL) when
  `[networks].targets` includes tor / i2p / lokinet.
* `external_assets` — refuses any external-origin asset
  reference, period.
* `reader_safety` — confirms pages render without JS, custom
  fonts, cookies, or localStorage.

## Operational requirements beyond the build

The phase enforces the BUILD-side guarantees. The OPERATIONAL
side is on the operator:

1. **No third-party CDN anywhere in the deploy chain.** Caddy
   serves directly from local disk. No Cloudflare, no
   Cloudflare-as-CDN-only-no-MITM (Cloudflare still logs).
   No DNS-over-Cloudflare on the server resolver.
2. **No analytics — not even self-hosted.** Plausible.io,
   Matomo, Umami, even self-hosted versions of these still
   compose pings into rendered pages. Hunted-tier deploys do
   not measure traffic.
3. **No fonts from fonts.gstatic.com.** All fonts ship from
   the same origin or no custom font at all (the loom-tokens
   default system-font stack is the answer).
4. **No images from third-party origins.** The `network_
   target_enforcement` phase catches script/link/img/fetch
   URLs; double-check `<picture>` `<source>` and CSS
   `background-image: url(...)` references in any custom CSS
   the operator wrote.
5. **Webfonts loaded from same-origin must be hashed in CSP.**
   Loom's `font-src` defaults handle this; custom themes
   that add fonts must update CSP `font-src` correspondingly.
6. **TLS configuration on the clearnet side** (when running
   clearnet+tor mirror): TLS 1.3 only, `ssl_protocols TLSv1.3`,
   no TLS 1.2 fallback. HSTS preload-eligible. OCSP must-staple.
7. **No third-party reverse proxy.** If the deploy is behind
   Cloudflare-the-CDN, hunted-tier ISN'T hunted-tier — every
   visitor's IP + headers route through Cloudflare's network.
   Use direct DNS to the server's IP.
8. **Tor side: keys backed up per `TOR_OPERATIONS.md` §4.**
   Vanguards (lite or addon per §5) running. DoS defenses on
   per §1.
9. **Server logging configured to drop the `RemoteAddr`** —
   Caddy can be configured with `log_remote_addr false` (or
   the equivalent matcher pattern) so access logs don't even
   contain the visitor's IP. Reduces what a subpoena can
   produce.
10. **No telemetry in any host-side daemon.** systemd-journald
    + journald-remote off. No Prometheus push to external
    aggregators. Local-only monitoring.

## Pre-publish verification checklist

Run this before pointing real users at a hunted-tier build:

```
[ ] cargo test -p forge-phases hunted_tier — all green
[ ] forge.toml [security] tier = "hunted"
[ ] forge.toml [noscript_strict] enabled = true
[ ] forge.toml [reader_safety] strict = true
[ ] LOOM_NOSCRIPT_MODE=1 forge build — strict findings == 0
[ ] grep -rE '<script' static/  — zero hits (sanity beyond the phase)
[ ] grep -rEi 'analytics|gtag|matomo|cloudflare|cdn\.' static/  — zero hits
[ ] grep -rE 'https://[^/]*\.(google|cloudfront|cdn|fastly)' static/  — zero hits
[ ] Tor Browser at Safest security level — every page renders + every link works
[ ] crawler-runner against the live target — zero strict findings on
    network_target_enforcement, reader_safety, noscript_strict,
    hunted_tier, meta_refresh
[ ] DNS: A/AAAA points DIRECTLY to the server (no Cloudflare in front)
[ ] Caddy: ssl_protocols TLSv1.3 only
[ ] Caddy: log_remote_addr false (or matcher pattern)
[ ] systemd: journald-remote DISABLED, no Prometheus push
[ ] Server-side resolver: NOT routed via Cloudflare-DNS / Google-DNS
[ ] HSTS: max-age >= 31536000; preload; includeSubDomains
[ ] HSTS preload list: site submitted at https://hstspreload.org/
[ ] Onion-Location header: configured (if clearnet+tor co-publish)
[ ] Tor: vanguards-lite confirmed running (Tor 0.4.7+ built-in)
    OR vanguards addon installed + running per TOR_OPERATIONS.md §5
[ ] Tor key backup: archive stored on encrypted cold media (NOT git)
[ ] Tor key backup verified: practice-restored on a throwaway VM
[ ] No third-party scripts injected by the host's CDN / WAF /
    application firewall (some hosts inject Cloudflare RUM by default)
```

A site passing all of these can be described as hunted-tier and
pointed at users whose threat model justifies the operational cost.

## Audit recipe — confirming hunted-tier compliance

```bash
# 1. Forge phase audit (build-side gates)
cd /path/to/site
LOOM_NOSCRIPT_MODE=1 ./target/release/forge build --strict

# 2. Static-asset external-origin scan (belt + braces over the
#    network_target_enforcement + external_assets phases)
grep -rEi 'https?://(?!localhost|127\.0\.0\.1)' static/ \
  | grep -v '#'  # exclude same-origin fragment refs
# Should print nothing.

# 3. Body-text leak scan (the hunted_tier phase already does this;
#    this is a developer pre-check)
grep -rE 'localStorage\.|sessionStorage\.|document\.cookie|navigator\.(geolocation|usb|bluetooth|serial)|canvas\.toDataURL|WebGLRenderingContext' static/
# Should print nothing.

# 4. Crawler runtime audit against the running .onion
cd /path/to/PlausiDen-Crawler
RUST_LOG=info ./target/release/crawler-runner journey-from-config \
  --config crawler-journey-hunted-tier.yml
# Should produce strict==0 across noscript_strict, reader_safety,
# hunted_tier, network_target_enforcement, meta_refresh.

# 5. Caddy access-log inspection — confirm no remote-addr logged
sudo tail /var/log/caddy/access.log | head
# Should NOT contain visitor IPs in the request_ip / common_log fields.
```

## What hunted-tier looks like in practice

For a site that passes:

* HTML carries zero `<script>` tags.
* CSP `Content-Security-Policy: default-src 'self'; script-src
  'none'; style-src 'self' 'sha256-...'; img-src 'self' data:;
  font-src 'self'; connect-src 'none'; require-trusted-types-for
  'script'; trusted-types 'none'; frame-ancestors 'none'`
* No fonts loaded from external origins.
* No analytics pings, no Sentry, no telemetry, no anything.
* Theme switching via CSS-only `:has()` radio fieldset
  (commit 7b82814).
* Forms work via plain `method="post"` to same-origin handlers.
* Modals via native `<dialog>` chrome with `<form method="dialog">`
  close buttons.
* `.onion` mirror on the same `static/` bundle, Vanguards-lite
  active, key backup verified.
* TLS 1.3 only on the clearnet side, HSTS preload-eligible,
  no third-party CDN in the deploy chain.
* Server-side: no remote-addr logging, no journald-remote,
  no external metric pushes.

The site is genuinely uninteresting to compromise — there's
nothing client-state-bound to recover, no third-party
correlation surface, no analytics aggregator to subpoena.

## Future work

* **`hunted_tier` Crawler axis** — runtime probe that confirms
  the rendered DOM has zero behavioral JS surfaces (no inline
  event handlers, no `<script>` blocks, no `<link rel="preload"
  as="script">`). Today the audit is filesystem-grep over
  `static/*.html`; runtime would catch the rare case where a
  page passes the static check but executes loaded JS via some
  exotic path.
* **CSP self-audit** — Forge phase that parses its own emitted
  CSP and asserts hunted-tier-mandatory directives are present.
  Today the CSP is rendered by Loom; phase-side verification
  would catch a Loom regression that loosens the policy.
* **DNS resolver audit** — host-side check that
  `/etc/resolv.conf` doesn't route via Cloudflare-DNS /
  Google-DNS / Quad9 (each of which can correlate the
  server-side lookups with timing of inbound .onion descriptor
  publishes). Add to the weekly host-audit cron.
