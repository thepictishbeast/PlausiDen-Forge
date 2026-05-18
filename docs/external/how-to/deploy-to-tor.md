# How-to: deploy to a Tor v3 hidden service

> **Diátaxis tier: How-to guide** — task-oriented. This page
> assumes you already have a working PlausiDen site (see
> [Tutorial 01](../tutorials/01-first-site.md) if not) and a
> Tor daemon running locally with a HiddenServiceDir configured.

## What you'll do

* Declare a `tor-onion` deploy target.
* Point the platform at your Tor `HiddenServiceDir`.
* Run `forge deploy` and verify the published `.onion` URL.

## Prerequisites

* Tor daemon installed + running with a HiddenServiceDir entry
  in `torrc`. Example:

  ```
  HiddenServiceDir /var/lib/tor/my-site/
  HiddenServicePort 80 127.0.0.1:8080
  ```

* Read access to the HiddenServiceDir for the user running Forge.

## Step 1: Add the target to your config

In `deploy.toml` (workspace root):

```toml
[[targets]]
id = "tor-main"
class = "tor-onion"

  [targets.extra]
  hidden_service_dir = "/var/lib/tor/my-site"
  web_root = "/var/www/my-site"
  virtual_port = 80
  target_port = 8080
  control_port = 9051
```

Field reference: [DeployTarget](../reference/deploy-target.md).

## Step 2: Verify the target before deploying

```bash
cargo run -q -p forge-cli -- deploy validate tor-main
```

Output you should see if everything is wired up:

```
deploy-validate tor-main:
  adapter   tor-onion
  hostname  abcdef…onion (resolved from hidden_service_dir/hostname)
  publish   not implemented (config check only)
```

If hostname says `(unresolved)`, Tor hasn't generated the keys
yet. Restart the Tor daemon and wait ~30s.

## Step 3: Deploy

```bash
cargo run -q -p forge-cli -- deploy push tor-main
```

This calls into the `deploy-onion` adapter, which:

1. Re-validates the target config.
2. Reads the `.onion` hostname from
   `hidden_service_dir/hostname`.
3. Reports the public URL.

> **Note**: the content-publish half of `deploy-onion` lands in a
> follow-up. Today the adapter validates + reports; copying the
> built site into the web root is a manual step until the publish
> half is wired (see the deploy-onion crate docs for status).

## Verifying anonymity guarantees

Run `forge deploy security-rating tor-main`. You should see:

```
tor-main      tor-onion       16/20   Excellent
  reader_anonymity      Strong
  publisher_anonymity   Strong
  traffic_observability Low
  censorship_resistance High
  content_addressed     false
  uses_standard_tls     false
```

The 16/20 score matches the platform's typed
[SecurityProfile::tor_onion_baseline](../reference/security-profile.md).

## Common pitfalls

* **`hidden_service_dir` exists but `/hostname` file missing**:
  Tor needs to be running with that directory in `torrc` AND
  successfully bootstrapped. Check `journalctl -u tor` for
  errors.
* **Permission denied reading the directory**: Tor creates
  `HiddenServiceDir` with mode `0700`. Add the Forge-user to the
  `_tor`/`debian-tor` group or run Forge as that user.
* **The `.onion` hostname changed after a server move**: Tor's
  HiddenServiceDir holds the long-lived secret key. If you lost
  the directory, you lost the address — that's by design.
