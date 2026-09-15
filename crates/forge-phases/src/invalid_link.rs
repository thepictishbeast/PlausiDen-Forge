//! `invalid_link` — strict-fail any HTML carrying the renderer's own
//! dead-link sentinel, `href="#invalid-link"`.
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
//! False-positive guard: matches only inside an `href` attribute
//! value, so prose or a code sample mentioning the string does not
//! trip it.

use std::path::{Path, PathBuf};

use forge_core::{BuildCtx, BuildError, Finding, Phase};

/// The literal the renderer substitutes for a refused `href`.
const SENTINEL: &str = "#invalid-link";

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
            let n = count_dead_hrefs(&body);
            if n > 0 {
                let name = display_name(&ctx.static_dir, &path);
                findings.push(Finding::strict(
                    self.name(),
                    name,
                    &format!(
                        "{n} link(s) render href=\"{SENTINEL}\" — the renderer refused the \
                         authored URL and substituted its dead-link sentinel, so the link \
                         is inert in production. Either the URL is unsafe and should be \
                         removed, or it uses a scheme the validator does not admit at that \
                         call site (mailto: and tel: need is_safe_contact_href, not \
                         is_safe_url)."
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

/// Count `href` attribute values equal to the sentinel. Quote style is
/// normalised first so single-quoted attributes are not a blind spot.
fn count_dead_hrefs(body: &str) -> usize {
    let lower = body.to_ascii_lowercase();
    let mut count = 0;
    for pat in [
        format!("href=\"{SENTINEL}\""),
        format!("href='{SENTINEL}'"),
    ] {
        count += lower.matches(&pat).count();
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    // NB: these fixtures embed `href="#invalid-link"`, and the `"#`
    // inside would close an `r#"..."#` literal early. Hence `r##`.

    #[test]
    fn counts_dead_hrefs_in_both_quote_styles() {
        assert_eq!(count_dead_hrefs(r##"<a href="#invalid-link">x</a>"##), 1);
        assert_eq!(count_dead_hrefs(r##"<a href='#invalid-link'>x</a>"##), 1);
        assert_eq!(
            count_dead_hrefs(r##"<a href="#invalid-link">a</a><a href="#invalid-link">b</a>"##),
            2
        );
    }

    #[test]
    fn the_exact_markup_that_shipped_is_caught() {
        // Copied from the live contact page, 2026-09-09.
        let shipped = r##"<a class="loom-contact-strip__item kind-email" href="#invalid-link" data-backend="contact-email"><span class="loom-contact-strip__label">william@plausiden.com</span></a>"##;
        assert_eq!(count_dead_hrefs(shipped), 1);
    }

    #[test]
    fn ignores_the_string_outside_an_href() {
        // Prose and code samples must not trip the gate.
        assert_eq!(count_dead_hrefs("<p>we emit #invalid-link on refusal</p>"), 0);
        assert_eq!(count_dead_hrefs(r##"<code>href=#invalid-link</code>"##), 0);
        assert_eq!(count_dead_hrefs(r##"<a href="/invalid-link">real page</a>"##), 0);
    }

    #[test]
    fn a_clean_page_is_silent() {
        assert_eq!(
            count_dead_hrefs(r##"<a href="mailto:a@b.com">a@b.com</a><a href="/x">x</a>"##),
            0
        );
    }

    #[test]
    fn case_insensitive_on_attribute_name() {
        assert_eq!(count_dead_hrefs(r##"<a HREF="#INVALID-LINK">x</a>"##), 1);
    }

    #[test]
    fn walk_reaches_pages_in_subdirectories() {
        // The blind spot this phase exists to close. Under
        // `clean_urls = true` every page but the home page is at
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
        // A non-HTML file must be ignored.
        std::fs::write(nested.join("notes.txt"), "href=\"#invalid-link\"").expect("write txt");

        let files = walk_html_recursive(&root, "test").expect("walk");
        assert_eq!(files.len(), 2, "expected both html files, got {files:?}");
        let total: usize = files.iter().map(|(_, b)| count_dead_hrefs(b)).sum();
        assert_eq!(total, 1, "the nested dead link was not seen");

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn missing_static_dir_is_an_empty_walk_not_an_error() {
        let missing = std::env::temp_dir().join("forge-invalid-link-does-not-exist");
        let files = walk_html_recursive(&missing, "test").expect("must not error");
        assert!(files.is_empty());
    }
}
