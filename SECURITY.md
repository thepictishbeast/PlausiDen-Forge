# Security policy

This is the security policy for the PlausiDen platform —
Forge + Loom + Crawler + Annotator + CMS.

## Reporting a vulnerability

**Email**: <security@plausiden.com>

If you believe you have found a security vulnerability in any
of the PlausiDen repositories, please report it privately by
email. Do not file a public issue.

A response acknowledging the report will be sent within 48
hours. Triage + remediation follows the timeline in [Response
times](#response-times).

If the vulnerability needs end-to-end encrypted reporting,
include a PGP-encrypted attachment using
[security-pgp.txt](./security-pgp.txt) (PGP key fingerprint
pinned in `static/.well-known/`). Plain mail is also acceptable
for low-severity reports.

## Scope

In scope:
- Code published in the five PlausiDen repos under
  [thepictishbeast](https://github.com/thepictishbeast).
- Operator-facing surfaces: CLI, server endpoints, admin UI,
  deploy adapters.
- Supply-chain configurations: `deny.toml`, GH Actions
  workflows, attestation chains.

Out of scope:
- Third-party services consumed via Cargo deps (report to the
  upstream maintainer).
- Operator misconfigurations (e.g. weak SSH keys, exposed
  control ports). Those are operator-side and not platform
  bugs.
- Findings from automated scanners without a working proof of
  exploitation.

## Severity classification

We use a four-tier rubric tied to CVSS 4.0:

| Tier | CVSS | Examples |
|---|---|---|
| **P0** | 9.0–10.0 | RCE, auth bypass, key compromise, tenant isolation break |
| **P1** | 7.0–8.9  | XSS in admin UI, privilege escalation, signature forgery |
| **P2** | 4.0–6.9  | Information disclosure, weakly-protected secrets in logs |
| **P3** | 0.1–3.9  | Outdated header advice, minor cookie attribute issues |

## Response times

| Severity | Acknowledgment | Triage decision | Patch + disclosure |
|---|---|---|---|
| P0 | 24 hours | 48 hours | 7 days |
| P1 | 48 hours | 5 business days | 30 days |
| P2 | 5 business days | 15 business days | 90 days |
| P3 | 10 business days | 30 business days | best-effort, next scheduled release |

We commit to coordinated disclosure: the reporter is notified
when a patch is available, and the public advisory ships
together with the patch.

## Bug bounty

PlausiDen runs a **structured bug bounty** program for verified
findings in scope:

| Severity | Reward range (USD) |
|---|---|
| P0 | $5,000 – $20,000 |
| P1 | $1,000 – $5,000 |
| P2 | $250 – $1,000 |
| P3 | $50 – $250 (operator-recognition) |

Final amount within range is determined by impact + exploitability
+ report quality + coordination behavior. We pay via the
operator's choice of bank transfer, PayPal, or equivalent.

Reports must:
- Include a reproducible proof of concept.
- Affect a current main-branch commit (not a published-but-since-
  patched older release).
- Demonstrate impact (not just theoretical risk).

We do not negotiate with extortion attempts. Reports submitted
with payment demands are forwarded to law enforcement.

## Hall of thanks

Reporters who chose to be acknowledged appear here. Email at
report time if you want public attribution; the default is
anonymous.

<!-- 2026-05-18: list seeded. Entries added as reports verify. -->

* _(reports listed as they verify)_

## Operational security commitments

The platform commits to:

* **Supply chain**: cargo-deny advisory checks, cargo-audit,
  CycloneDX SBOM emission, build provenance attestation. See
  `.github/workflows/supply-chain.yml`.
* **Cryptographic signing**: every artifact carries an
  Ed25519 signature today + post-quantum hybrid (Ed25519 +
  ML-DSA-65) per task #65. Verification dispatches on
  algorithm slug; old verifiers fail closed on unknown
  algorithms.
* **Hash-chained audit logs**: every administrative action
  records an immutable entry per
  `observability-core::AuditChain` (task #69). Tampering with
  historical entries fails verification.
* **Reproducible builds**: `cargo install --locked` everywhere
  CI installs tooling; pinned tool versions in
  `.github/workflows/supply-chain.yml`.
* **GH Actions SHA pinning**: all third-party actions are
  pinned to commit SHAs, not version tags (task #74). A
  malicious tag retag can't silently land in CI.

## Coordinated disclosure & embargoes

For high-severity findings, we will work with the reporter on
a coordinated embargo window. Default 90 days from triage
acknowledgment; longer if patch development requires it.
Embargoes are NOT used to suppress reports — they're for
giving the platform time to ship a fix.

If we miss an agreed embargo window without a fix in place, the
reporter is free to disclose publicly. We commit to keeping the
reporter informed of progress at least weekly during an embargo.

## Pen test + red team

We engage external pen-testing firms annually for the deploy
adapter set (Tor / I2P / IPFS / Gemini / Lokinet) and the admin
auth path (#60 cms-admin-auth). Red-team exercises run
quarterly against the manifest-attest + observability-core
chains. Results inform the next iteration's hardening
priorities; report summaries (with sensitive details redacted)
are linked from the audit posture page once SOC 2 readiness
(#73) lands.

## What we don't promise

* We don't promise a fix for every reported issue. P3 findings
  may be deferred to a future release.
* We don't promise specific reward amounts. The ranges above
  are guidance; final amount is at our discretion.
* We don't promise bounty for issues affecting EOL versions or
  versions we never shipped.

## Updates to this policy

This document is versioned in the PlausiDen-Forge repo. Material
changes get announced via the same channels as security
advisories. Reports filed against a previous version of this
policy are honored under the rules in force at submission time.

---

*Last updated: 2026-05-18*
