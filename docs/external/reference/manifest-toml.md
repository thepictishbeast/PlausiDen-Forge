# Reference: manifest.toml

> **Diátaxis tier: Reference** — information-oriented. The canonical
> shape of every field PlausiDen's [`PlatformManifest`] type
> accepts. Generated from the Rust source-of-truth; if this page
> drifts from `manifest-core/src/lib.rs`, the Rust types are
> authoritative.

[`PlatformManifest`]: https://github.com/thepictishbeast/PlausiDen-Forge/blob/main/crates/manifest-core/src/lib.rs

## Top-level fields

```toml
schema-version = "1"           # string. Manifest schema version.
platform       = "my-site"     # string. Human-readable platform id.

[[capabilities]]               # zero or more capability entries
# … see Capability section

[[phases]]                     # zero or more phase entries
# … see PhaseDescriptor section

[[backends]]                   # zero or more backend entries
# … see BackendDescriptor section

[coverage]                     # optional coverage policy
# … see CoveragePolicy section
```

### `schema-version` (string)

Defaults to `"1"` if omitted. Bumped only on breaking changes to
the manifest's JSON wire shape.

### `platform` (string, required)

Human-readable platform identifier. Used by `manifest-codegen` as
the `MANIFEST_PLATFORM` constant downstream crates `include!()`.

## Capability

One platform capability. Capabilities are the smallest unit the
manifest tracks; the CI coverage gate refuses to merge when a
declared capability has zero handlers / UI / tests / docs.

```toml
[[capabilities]]
id          = "auth-login"                # required. kebab-case.
summary     = "User authentication"       # required. one-line.
ownership   = "forge"                     # required. closed enum.
handlers    = ["forge-phases::auth"]      # optional. handler refs.
ui          = ["cms-admin::auth"]         # optional. UI refs.
tests       = ["forge-phases::auth::tests::ok"]  # optional.
docs        = ["docs/external/explanation/auth.md"]  # optional.
```

### `id` (kebab-case string, required)

Must match `^[a-z][a-z0-9]*(-[a-z0-9]+)*$`. Length 1..=64. Two
capabilities cannot share an id.

### `ownership` (closed enum, required)

Which subsystem owns this capability:

| Value | Subsystem |
|---|---|
| `forge` | PlausiDen-Forge — build pipeline + audit |
| `loom` | PlausiDen-Loom — typed design primitives |
| `crawler` | PlausiDen-Crawler — runtime audit |
| `annotator` | PlausiDen-Annotator — desktop UX capture |
| `cms` | PlausiDen-CMS — typed CMS + admin editor |

### `handlers` / `ui` / `tests` / `docs` (string arrays, optional)

Free-form references — typically Rust module paths or filesystem
paths. The CI coverage gate counts them by length (default min 1
per capability, all four lists).

## PhaseDescriptor

A Forge build phase. Projected from `phases.toml` (workspace
root) at build time.

```toml
[[phases]]
id              = "p-auth"        # required. kebab-case.
summary         = "auth phase"    # required. one-line.
implements      = "auth-login"    # optional. capability id.
default-severity = "strict"       # optional. info|warn|strict.
depends-on      = []              # optional. phase id list.
```

### `default-severity` (closed enum, optional)

| Value | Behaviour |
|---|---|
| `info` | Information only — never gates a build. |
| `warn` | Visible in reports; doesn't gate. Default. |
| `strict` | Gates the build on detection. |

### `depends-on` (kebab-case string array, optional)

Phase ids that MUST run before this one. Forge topo-sorts the
pipeline using these edges. Cycle detection refuses the build.

## BackendDescriptor

A runtime backend endpoint. Projected from `backends.toml` at
build time.

```toml
[[backends]]
id          = "post-skill"        # required. kebab-case.
summary     = "Submit a skill challenge"  # required.
implements  = "challenges"        # optional. capability id.
route       = "/api/skills"       # optional. HTTP route.
method      = "POST"              # optional. HTTP method.
```

## CoveragePolicy

What "covered" means for the CI gate (task #33).

```toml
[coverage]
min-handlers = 1     # default 1
min-ui       = 1     # default 1
min-tests    = 1     # default 1
min-docs     = 1     # default 1
exempt       = []    # optional. capability ids exempt from gate.
```

Use `exempt` sparingly — every exemption is technical debt the
next audit pass has to justify.

## Validation

Parse + structural validation runs automatically when the
manifest is loaded:

* Duplicate capability/phase/backend ids → typed error.
* `implements` reference to unknown capability → typed error.
* `depends-on` cycle → typed error from the topo-sort.

Run on demand via:

```bash
forge manifest validate
```

Exit 0 on clean, exit 1 on any gate violation.
