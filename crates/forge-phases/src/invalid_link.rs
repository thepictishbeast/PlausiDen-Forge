//! `invalid_link` — strict-fail any HTML carrying one of the renderer's
//! own dead-link sentinels: the `#invalid-` family, in any attribute
//! the browser would actually follow.
//!
//! loom-cms-render substitutes that sentinel whenever an `href` fails
//! validation, on the reasoning that a build phase will catch it. No
//! phase did. The result shipped: william-armstrong.com served a
//! contact page whose only channel was
//! `<a href="#invalid-link">william@plausiden.com</a>` — the address
//! visible as text, the link inert, every one of the other 79 phases
//! green.
//!
//! The sentinel means one of two things, and both are build failures:
//! the author wrote a genuinely unsafe URL, or the validator refuses a
//! scheme the page legitimately needs. This phase cannot tell them
//! apart and should not try — it says a link is dead and names the
//! file, which is the part nobody noticed for months.
//!
//! False-positive guard, and it is implemented rather than merely
//! claimed: a match counts only when it sits INSIDE a start tag and
//! opens the whole value of `href`, `action`, `src` or `formaction`.
//! So prose mentioning a sentinel, a rendered code sample (whose
//! angle brackets arrive escaped), a real URL that merely contains
//! the text, and a non-URL attribute all pass silently.

use std::path::{Path, PathBuf};

use forge_core::{BuildCtx, BuildError, Finding, Phase};

/// The prefix every dead-link sentinel shares. Matching the FAMILY is
/// the point: the renderer emits `#invalid-link`, `#invalid-cta`,
/// `#invalid-action`, `#invalid-form-action`, `#invalid-endpoint`,
/// `#invalid-card`, `#invalid-social-link`, `#invalid-nav-link` and
/// `#invalid-lang-href`. A gate pinned to one of them is blind to the
/// other eight, which is exactly how the first cut of this phase
/// shipped.
const SENTINEL_PREFIX: &str = "#invalid-";

/// `invalid_link` phase.
#[derive(Debug, Default)]
pub struct InvalidLinkPhase;

impl Phase for InvalidLinkPhase {
    fn name(&self) -> &'static str {
        "invalid_link"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        // Deliberately NOT `html_walk::walk_html`: that helper reads
        // only `<static_dir>/*.html`, so under `clean_urls = true`
        // every page but the home page — which lives at
        // `static/<slug>/index.html` — is invisible to it. That blind
        // spot is why a dead link on /contact/ survived a 79-phase
        // build. This phase walks the tree.
        let files = walk_html_recursive(&ctx.static_dir, self.name())?;
        let mut findings = Vec::new();

        for (path, body) in files {
            let found = find_dead_sentinels(&body);
            if !found.is_empty() {
                let name = display_name(&ctx.static_dir, &path);
                let total: usize = found.iter().map(|(_, n)| n).sum();
                let detail = found
                    .iter()
                    .map(|(s, n)| format!("{s} x{n}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                findings.push(Finding::strict(
                    self.name(),
                    name,
                    &format!(
                        "{total} dead link(s): {detail}. The renderer refused the authored \
                         URL and substituted its dead-link sentinel, so the link is inert \
                         in production. Either the URL is unsafe and should be removed, or \
                         it uses a scheme the validator does not admit at that call site \
                         (mailto: and tel: need is_safe_contact_href, not is_safe_url)."
                    ),
                ));
            }
        }

        Ok(findings)
    }
}

/// Path shown in the finding, relative to the static dir so it reads
/// `contact/index.html` rather than an absolute path.
fn display_name(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

/// Every `*.html` under `static_dir`, at any depth, in deterministic
/// order. A missing directory is an empty walk, not an error — same
/// contract as `html_walk::walk_html`.
fn walk_html_recursive(
    static_dir: &Path,
    phase: &str,
) -> Result<Vec<(PathBuf, String)>, BuildError> {
    let mut found = Vec::new();
    let mut stack = vec![static_dir.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(it) => it,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                return Err(BuildError::Io {
                    context: format!("{phase}: read_dir {}", dir.display()),
                    source: e,
                });
            }
        };
        for entry in entries {
            let entry = entry.map_err(|e| BuildError::Io {
                context: format!("{phase}: dir entry under {}", dir.display()),
                source: e,
            })?;
            let p = entry.path();
            // Symlinks are not followed: a link pointing outside the
            // tree, or back into it, would take the walk somewhere the
            // build did not produce.
            let meta = entry.metadata().map_err(|e| BuildError::Io {
                context: format!("{phase}: metadata {}", p.display()),
                source: e,
            })?;
            if meta.is_dir() {
                stack.push(p);
            } else if meta.is_file() && p.extension().and_then(|s| s.to_str()) == Some("html") {
                let body = std::fs::read_to_string(&p).map_err(|e| BuildError::Io {
                    context: format!("{phase}: read {}", p.display()),
                    source: e,
                })?;
                found.push((p, body));
            }
        }
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(found)
}

/// Attributes whose value is a URL the browser will actually follow.
const ATTRS: [&str; 4] = ["href", "action", "src", "formaction"];

/// Find every dead-link sentinel, by matching the `#invalid-` FAMILY
/// rather than one literal.
///
/// The first cut of this phase matched only `href="#invalid-link"`.
/// The renderer emits NINE distinct sentinels across 81 sites, and
/// `#invalid-cta` alone accounts for 34 -- more than the one that was
/// caught. So the gate written because a dead link shipped was blind
/// to most of the ways a dead link ships, including the
/// `call_to_action` and `<form action>` sentinels that the very commit
/// introducing this phase added fresh instances of.
///
/// Returns each distinct sentinel found with its count, so a finding
/// can name what it actually saw.
fn find_dead_sentinels(body: &str) -> Vec<(String, usize)> {
    let lower = body.to_ascii_lowercase();
    let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();

    for (idx, _) in lower.match_indices(SENTINEL_PREFIX) {
        // The sentinel must sit inside a start tag. Prose or a code
        // sample that MENTIONS one is documentation, not a dead link
        // -- and this phase's own commit message quotes one. "Inside a
        // tag" means the nearest preceding '<' comes after the nearest
        // preceding '>'. In escaped markup a sample's '<' arrives as
        // `&lt;`, so rendered documentation correctly fails this test.
        let lt = lower[..idx].rfind('<');
        let gt = lower[..idx].rfind('>');
        let inside_tag = match (lt, gt) {
            (Some(l), Some(g)) => l > g,
            (Some(_), None) => true,
            _ => false,
        };
        if !inside_tag {
            continue;
        }

        // It must open the value of a URL-bearing attribute, so a
        // sentinel appearing mid-URL or in some other attribute is
        // not counted.
        let before = &lower[..idx];
        if !ATTRS
            .iter()
            .any(|a| before.ends_with(&format!("{a}=\"")) || before.ends_with(&format!("{a}='")))
        {
            continue;
        }

        // Read to the closing quote to recover the whole sentinel.
        let rest = &lower[idx..];
        let end = rest.find(['"', '\'']).unwrap_or(rest.len());
        let token = &rest[..end];
        if token.len() > SENTINEL_PREFIX.len() {
            *counts.entry(token.to_owned()).or_default() += 1;
        }
    }

    counts.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // NB: fixtures embed `href="#invalid-link"`, and the `"#` inside
    // would close an `r#"..."#` literal early. Hence `r##`.

    fn total(body: &str) -> usize {
        find_dead_sentinels(body).iter().map(|(_, n)| n).sum()
    }

    #[test]
    fn counts_dead_hrefs_in_both_quote_styles() {
        assert_eq!(total(r##"<a href="#invalid-link">x</a>"##), 1);
        assert_eq!(total(r##"<a href='#invalid-link'>x</a>"##), 1);
        assert_eq!(
            total(r##"<a href="#invalid-link">a</a><a href="#invalid-link">b</a>"##),
            2
        );
    }

    #[test]
    fn the_exact_markup_that_shipped_is_caught() {
        // Copied from the live contact page, 2026-09-09.
        let shipped = r##"<a class="loom-contact-strip__item kind-email" href="#invalid-link" data-backend="contact-email"><span class="loom-contact-strip__label">william@plausiden.com</span></a>"##;
        assert_eq!(total(shipped), 1);
    }

    #[test]
    fn catches_the_whole_sentinel_family_not_just_invalid_link() {
        // REGRESSION: the first cut matched only `#invalid-link` and
        // was blind to the other eight. `#invalid-cta` has MORE
        // emission sites than the one it caught, and the commit that
        // introduced this phase added fresh call_to_action and
        // <form action> instances it could not see.
        for s in [
            "#invalid-link",
            "#invalid-cta",
            "#invalid-action",
            "#invalid-form-action",
            "#invalid-endpoint",
            "#invalid-card",
            "#invalid-social-link",
            "#invalid-nav-link",
            "#invalid-lang-href",
        ] {
            let html = format!(r#"<a href="{s}">x</a>"#);
            assert_eq!(total(&html), 1, "missed sentinel {s}");
        }
    }

    #[test]
    fn catches_sentinels_on_every_url_bearing_attribute() {
        assert_eq!(total(r##"<form action="#invalid-form-action">"##), 1);
        assert_eq!(total(r##"<img src="#invalid-card">"##), 1);
        assert_eq!(total(r##"<button formaction="#invalid-action">"##), 1);
    }

    #[test]
    fn names_what_it_found() {
        let html = r##"<a href="#invalid-cta">a</a><form action="#invalid-form-action"></form>"##;
        let found = find_dead_sentinels(html);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found.iter().any(|(s, n)| s == "#invalid-cta" && *n == 1));
        assert!(found
            .iter()
            .any(|(s, n)| s == "#invalid-form-action" && *n == 1));
    }

    #[test]
    fn ignores_the_string_outside_a_tag() {
        // Prose and RENDERED code samples must not trip the gate. In
        // rendered markup a sample's angle bracket arrives escaped,
        // so the sentinel sits in text, not inside a start tag.
        assert_eq!(total("<p>we emit #invalid-link on refusal</p>"), 0);
        assert_eq!(
            total(r##"<code>&lt;a href="#invalid-link"&gt;</code>"##),
            0
        );
        assert_eq!(total(r##"<code>href=#invalid-link</code>"##), 0);
        assert_eq!(total(r##"<a href="/invalid-link">real page</a>"##), 0);
    }

    #[test]
    fn ignores_a_sentinel_that_is_not_the_whole_attribute_value() {
        // A real URL that merely contains the text is not a dead link.
        assert_eq!(total(r##"<a href="/docs/#invalid-link-policy">x</a>"##), 0);
        assert_eq!(total(r##"<a data-note="#invalid-link">x</a>"##), 0);
    }

    #[test]
    fn a_clean_page_is_silent() {
        assert_eq!(
            total(r##"<a href="mailto:a@b.com">a@b.com</a><a href="/x">x</a>"##),
            0
        );
    }

    #[test]
    fn case_insensitive_on_attribute_and_sentinel() {
        assert_eq!(total(r##"<a HREF="#INVALID-LINK">x</a>"##), 1);
    }

    #[test]
    fn walk_reaches_pages_in_subdirectories() {
        // Under `clean_urls = true` every page but the home page is at
        // `static/<slug>/index.html`; a non-recursive walk sees only
        // `static/index.html` and reports a clean build regardless.
        let root = std::env::temp_dir().join(format!(
            "forge-invalid-link-walk-{}",
            std::process::id()
        ));
        let nested = root.join("contact");
        std::fs::create_dir_all(&nested).expect("mkdir");
        std::fs::write(root.join("index.html"), "<a href=\"/x\">ok</a>").expect("write root");
        std::fs::write(
            nested.join("index.html"),
            "<a href=\"#invalid-link\">dead</a>",
        )
        .expect("write nested");
        std::fs::write(nested.join("notes.txt"), "href=\"#invalid-link\"").expect("write txt");

        let files = walk_html_recursive(&root, "test").expect("walk");
        assert_eq!(files.len(), 2, "expected both html files, got {files:?}");
        let n: usize = files.iter().map(|(_, b)| total(b)).sum();
        assert_eq!(n, 1, "the nested dead link was not seen");

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn missing_static_dir_is_an_empty_walk_not_an_error() {
        let missing = std::env::temp_dir().join("forge-invalid-link-does-not-exist");
        let files = walk_html_recursive(&missing, "test").expect("must not error");
        assert!(files.is_empty());
    }
}
