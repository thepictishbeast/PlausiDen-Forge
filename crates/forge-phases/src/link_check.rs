//! `link_check` — anchor fragment validation. Every `<a href="...#frag">`
//! must resolve to an `id="frag"` or `name="frag"` in the target file.
//!
//! Bash parity: `phase_link_check`. Despite the name, this phase
//! does NOT walk the open web — external (http/https) hrefs are
//! deliberately skipped because link-rot detection across third-
//! party endpoints is too flaky for build-time CI. Network-aware
//! link checking lives in a separate periodic job (queued).
//!
//! Catches:
//!
//! * Same-page jumps (`#contact`) where the target id was renamed
//! * Cross-page jumps (`/about.html#team`) where the target file
//!   exists but the section was removed
//! * Skip-link / table-of-contents typos
//!
//! Out of scope (covered by other phases):
//!
//! * Target file existence — `unbuilt_route` checks this
//! * External 2xx — periodic job
//! * Anchor accessibility — `a11y_landmarks` / axe

use std::collections::BTreeSet;

use forge_core::{BuildCtx, BuildError, Finding, Phase};

use crate::html_walk::{walk_html, HtmlFile};

/// `link_check` phase.
#[derive(Debug, Default)]
pub struct LinkCheckPhase;

impl Phase for LinkCheckPhase {
    fn name(&self) -> &'static str {
        "link_check"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let files = walk_html(&ctx.static_dir, self.name())?;
        // Pre-index: per-file set of in-document anchor names
        // (id="X" + <a name="X">). One pass over the corpus.
        let anchors_by_file: std::collections::BTreeMap<String, BTreeSet<String>> = files
            .iter()
            .map(|f| (f.name.clone(), collect_anchor_names(&f.body)))
            .collect();

        let mut findings = Vec::new();
        for file in &files {
            for href in extract_anchor_hrefs(&file.body) {
                let Some(target) = parse_target(&href) else {
                    continue;
                };
                if target.fragment.is_empty() {
                    // No fragment — unbuilt_route handles file existence.
                    continue;
                }
                let target_file_name = match target.file.as_deref() {
                    Some(name) => name,
                    None => &file.name, // same-page #frag
                };
                let Some(anchors) = anchors_by_file.get(target_file_name) else {
                    // Target file doesn't exist — unbuilt_route's job to
                    // flag, not link_check's. We silently skip.
                    continue;
                };
                if !anchors.contains(&target.fragment) {
                    findings.push(Finding::strict(
                        self.name(),
                        file.name.clone(),
                        format!(
                            "href=\"{href}\" → no #{} anchor found in {target_file_name}",
                            target.fragment
                        ),
                    ));
                }
            }
        }
        Ok(findings)
    }
}

/// What an anchor href resolved to.
struct Target {
    /// Target filename (relative to static/), or None for same-page.
    file: Option<String>,
    /// Fragment after `#`, never including the `#` itself.
    /// Empty string means "no fragment".
    fragment: String,
}

/// Parse an `<a href>` value into a structured Target. Returns
/// None for hrefs we deliberately don't validate (absolute URLs,
/// mailto, tel, javascript, query-only).
fn parse_target(href: &str) -> Option<Target> {
    if href.starts_with("http://")
        || href.starts_with("https://")
        || href.starts_with("//")
        || href.starts_with("mailto:")
        || href.starts_with("tel:")
        || href.starts_with("javascript:")
    {
        return None;
    }
    // Same-page fragment.
    if let Some(stripped) = href.strip_prefix('#') {
        return Some(Target {
            file: None,
            fragment: stripped.to_owned(),
        });
    }
    // Split on `#`. Drop query string before the fragment if any.
    let (file_part, fragment) = match href.find('#') {
        Some(i) => (&href[..i], href[i + 1..].to_owned()),
        None => (href, String::new()),
    };
    let file_part = match file_part.find('?') {
        Some(i) => &file_part[..i],
        None => file_part,
    };
    let file_norm = file_part.trim_start_matches("./").trim_start_matches('/');
    let file = if file_norm.is_empty() {
        Some("index.html".to_owned())
    } else {
        Some(file_norm.to_owned())
    };
    Some(Target { file, fragment })
}

/// Collect every anchorable name in a document body: `id="X"` and
/// `<a name="X">`. Returns the set of names.
///
/// Public so future phases (forge-html parser, fragment resolvers
/// in dynamic mode) can reuse the index without duplicating the
/// boundary-aware scanner.
///
/// BUG ASSUMPTION: the boundary check rejects `data-id="..."` as
/// a match for `id="..."` lookup, but accepts any attribute value
/// in `<a name="...">` (the legacy form is permissive — false
/// positives just add candidate anchors, which is harmless).
pub fn collect_anchor_names(body: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    // id="X" with a left-side boundary that's not alnum/_/-.
    let mut search = body;
    let mut offset = 0usize;
    while let Some(rel_idx) = search.find("id=\"") {
        let abs_idx = offset + rel_idx;
        let left_ok = if abs_idx == 0 {
            true
        } else {
            let prev = body.as_bytes()[abs_idx - 1] as char;
            !prev.is_ascii_alphanumeric() && prev != '-' && prev != '_'
        };
        let after = &search[rel_idx + "id=\"".len()..];
        if let Some(end) = after.find('"') {
            if left_ok {
                out.insert(after[..end].to_owned());
            }
            let advance = rel_idx + "id=\"".len() + end + 1;
            offset += advance;
            search = &search[advance..];
        } else {
            break;
        }
    }
    // <a name="X"> form (legacy).
    let mut search = body;
    while let Some(rel_idx) = search.find("name=\"") {
        let after = &search[rel_idx + "name=\"".len()..];
        // Left context check: must immediately follow `<a` plus
        // optional attributes — we don't enforce this strictly,
        // matches all `name="X"` in HTML. False-positive cost is
        // adding extra anchor candidates which is harmless.
        if let Some(end) = after.find('"') {
            out.insert(after[..end].to_owned());
            search = &after[end + 1..];
        } else {
            break;
        }
    }
    out
}

/// Pull every `<a ... href="...">` href value (substring scan).
fn extract_anchor_hrefs(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut search = body;
    while let Some(idx) = search.find("<a") {
        let after = &search[idx..];
        let Some(close) = after.find('>') else {
            break;
        };
        let tag = &after[..close];
        if let Some(href) = extract_attr(tag, "href=\"") {
            out.push(href);
        }
        search = &after[close + 1..];
    }
    out
}

fn extract_attr(tag: &str, prefix: &str) -> Option<String> {
    let idx = tag.find(prefix)?;
    let after = &tag[idx + prefix.len()..];
    let end = after.find('"')?;
    Some(after[..end].to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_same_page_fragment() {
        let t = parse_target("#contact").unwrap();
        assert!(t.file.is_none());
        assert_eq!(t.fragment, "contact");
    }

    #[test]
    fn parse_cross_page_fragment() {
        let t = parse_target("/about.html#team").unwrap();
        assert_eq!(t.file.as_deref(), Some("about.html"));
        assert_eq!(t.fragment, "team");
    }

    #[test]
    fn parse_no_fragment() {
        let t = parse_target("/about.html").unwrap();
        assert_eq!(t.file.as_deref(), Some("about.html"));
        assert_eq!(t.fragment, "");
    }

    #[test]
    fn parse_root_with_fragment() {
        let t = parse_target("/#hero").unwrap();
        assert_eq!(t.file.as_deref(), Some("index.html"));
        assert_eq!(t.fragment, "hero");
    }

    #[test]
    fn parse_skips_absolute() {
        assert!(parse_target("https://example.com").is_none());
        assert!(parse_target("mailto:x@y").is_none());
        assert!(parse_target("javascript:void(0)").is_none());
    }

    #[test]
    fn collect_anchor_names_basic() {
        let body = r#"<div id="hero"></div><div id="footer"></div>"#;
        let names = collect_anchor_names(body);
        assert!(names.contains("hero"));
        assert!(names.contains("footer"));
    }

    #[test]
    fn collect_anchor_names_skips_data_id() {
        let body = r#"<div id="real"></div><div data-id="ignored"></div>"#;
        let names = collect_anchor_names(body);
        assert!(names.contains("real"));
        assert!(!names.contains("ignored"));
    }

    #[test]
    fn collect_anchor_names_legacy_a_name() {
        let body = r#"<a name="legacy">old anchor</a>"#;
        let names = collect_anchor_names(body);
        assert!(names.contains("legacy"));
    }
}
