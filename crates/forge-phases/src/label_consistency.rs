//! `label_consistency` — every (kind, key) pair (where kind is
//! `a` keyed on `href` or `button` keyed on `data-backend`) must
//! carry exactly one distinct visible label across the whole
//! site, UNLESS every element in the group declares
//! `data-loom-poly-action="true"` to opt out (T513).
//!
//! Bash parity: `phase_label_consistency`. Owner-surfaced bug it
//! catches: nav link said "Post a Skill" while CTA button said
//! "Post skill" — same destination, two labels.
//!
//! T513 polymorphism doctrine:
//!
//! * **All elements opt-out** → silent. Intentional polymorphic
//!   action (e.g. one `data-backend="list-challenges"` wired to
//!   N filter buttons whose labels are filter modifiers).
//! * **Partial opt-out** → `Warn`. Annotation is incomplete; the
//!   author meant to declare polymorphism but missed an element.
//! * **No opt-out** → `Strict`. Real label-drift bug.

use std::collections::BTreeMap;

use forge_core::{BuildCtx, BuildError, Finding, Phase};

use crate::html_walk::walk_html;

/// `label_consistency` phase.
#[derive(Debug, Default)]
pub struct LabelConsistencyPhase;

/// One captured element (anchor or button) with the data we need
/// for grouping + opt-out decisions.
struct ElementHit {
    label: String,
    file: String,
    has_poly_optout: bool,
}

impl Phase for LabelConsistencyPhase {
    fn name(&self) -> &'static str {
        "label_consistency"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let files = walk_html(&ctx.static_dir, self.name())?;
        // (kind, key) -> Vec<ElementHit>. Keep Vec not BTreeMap so
        // we can compute opt-out coverage later.
        let mut groups: BTreeMap<(&'static str, String), Vec<ElementHit>> = BTreeMap::new();

        for file in &files {
            for (href, inner, optout) in extract_anchor(&file.body) {
                let label = normalize_label(&inner);
                if label.is_empty() || label.starts_with('▶') {
                    continue;
                }
                groups.entry(("a", href)).or_default().push(ElementHit {
                    label,
                    file: file.name.clone(),
                    has_poly_optout: optout,
                });
            }
            for (backend, inner, optout) in extract_button(&file.body) {
                let label = normalize_label(&inner);
                if label.is_empty() {
                    continue;
                }
                groups
                    .entry(("button", backend))
                    .or_default()
                    .push(ElementHit {
                        label,
                        file: file.name.clone(),
                        has_poly_optout: optout,
                    });
            }
        }

        let mut findings = Vec::new();
        for ((kind, key), hits) in &groups {
            // Distinct labels.
            let mut label_counts: BTreeMap<&str, usize> = BTreeMap::new();
            for h in hits {
                *label_counts.entry(h.label.as_str()).or_insert(0) += 1;
            }
            if label_counts.len() <= 1 {
                continue; // single label — clean
            }
            // Opt-out coverage: how many elements declared the attribute.
            let opted: usize = hits.iter().filter(|h| h.has_poly_optout).count();
            let total = hits.len();
            let summary: Vec<String> = label_counts
                .iter()
                .map(|(label, count)| format!("\"{label}\" ({count}x)"))
                .collect();

            if opted == total {
                // Fully annotated polymorphic action — silent. We
                // could still emit an INFO log here for visibility;
                // for now silence is the contract.
                continue;
            } else if opted > 0 {
                findings.push(Finding::warn(
                    self.name(),
                    "static/",
                    format!(
                        "{kind}[{key}] — {} distinct labels with PARTIAL data-loom-poly-action ({opted}/{total} elements opted out): {} — annotate the remaining elements OR remove the attribute on all",
                        label_counts.len(),
                        summary.join(", ")
                    ),
                ));
            } else {
                findings.push(Finding::strict(
                    self.name(),
                    "static/",
                    format!(
                        "{kind}[{key}] — {} distinct labels: {} — declare data-loom-poly-action=\"true\" if intentional polymorphism, otherwise consolidate to one label",
                        label_counts.len(),
                        summary.join(", ")
                    ),
                ));
            }
        }
        Ok(findings)
    }
}

/// Extract every `<a href="..."> INNER </a>` as
/// `(href, inner_html, has_data_loom_poly_action)`.
fn extract_anchor(body: &str) -> Vec<(String, String, bool)> {
    extract_pair(body, "<a", "href=\"", "</a>")
}

/// Extract every `<button data-backend="..."> INNER </button>`.
fn extract_button(body: &str) -> Vec<(String, String, bool)> {
    extract_pair(body, "<button", "data-backend=\"", "</button>")
}

fn extract_pair(
    body: &str,
    open_tag: &str,
    attr_prefix: &str,
    close_tag: &str,
) -> Vec<(String, String, bool)> {
    let mut out = Vec::new();
    let mut search = body;
    while let Some(open_idx) = search.find(open_tag) {
        let rest = &search[open_idx..];
        let Some(open_end) = rest.find('>') else {
            break;
        };
        let tag_open = &rest[..open_end];
        let after_open = &rest[open_end + 1..];
        let Some(key) = extract_attr(tag_open, attr_prefix) else {
            search = after_open;
            continue;
        };
        // T513: detect the opt-out attribute. Accepts both
        // `data-loom-poly-action="true"` and the bare attribute
        // form `data-loom-poly-action`. Anything other than
        // "false" counts as opted-out.
        let optout_val = extract_attr(tag_open, "data-loom-poly-action=\"");
        let optout = match optout_val.as_deref() {
            Some("false") | Some("0") => false,
            Some(_) => true,
            None => tag_open.contains("data-loom-poly-action"),
        };
        let Some(close_idx) = after_open.find(close_tag) else {
            break;
        };
        let inner = &after_open[..close_idx];
        out.push((key, inner.to_owned(), optout));
        search = &after_open[close_idx + close_tag.len()..];
    }
    out
}

fn extract_attr(tag: &str, prefix: &str) -> Option<String> {
    let idx = tag.find(prefix)?;
    let after = &tag[idx + prefix.len()..];
    let end = after.find('"')?;
    Some(after[..end].to_owned())
}

/// Strip nested tags + entities + collapse whitespace.
fn normalize_label(inner_html: &str) -> String {
    let mut out = String::with_capacity(inner_html.len());
    let mut in_tag = false;
    for c in inner_html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    let decoded = out
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");
    let mut result = String::with_capacity(decoded.len());
    let mut last_was_space = false;
    for c in decoded.chars() {
        if c.is_whitespace() {
            if !last_was_space && !result.is_empty() {
                result.push(' ');
            }
            last_was_space = true;
        } else {
            result.push(c);
            last_was_space = false;
        }
    }
    result.trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_tags_and_collapses() {
        assert_eq!(
            normalize_label("  <span>Hello</span>  <em>World</em>  "),
            "Hello World"
        );
    }

    #[test]
    fn normalize_decodes_common_entities() {
        assert_eq!(normalize_label("Cats &amp; dogs"), "Cats & dogs");
    }

    #[test]
    fn extract_anchor_pulls_label_no_optout() {
        let body = r#"<a href="/x.html">Foo</a><a href="/y.html"><span>Bar</span></a>"#;
        let pairs = extract_anchor(body);
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0].0, "/x.html");
        assert_eq!(pairs[0].1, "Foo");
        assert!(!pairs[0].2);
    }

    #[test]
    fn extract_anchor_detects_polyaction_optout() {
        let body = r#"<a href="/" data-loom-poly-action="true">Home</a><a href="/">Logo</a>"#;
        let pairs = extract_anchor(body);
        assert_eq!(pairs.len(), 2);
        assert!(pairs[0].2, "first <a> has data-loom-poly-action");
        assert!(!pairs[1].2, "second <a> has no opt-out");
    }

    #[test]
    fn extract_anchor_polyaction_false_is_not_optout() {
        let body = r#"<a href="/x" data-loom-poly-action="false">x</a>"#;
        let pairs = extract_anchor(body);
        assert!(!pairs[0].2, "explicit false must NOT count as opt-out");
    }

    #[test]
    fn extract_button_polyaction_bare_attribute() {
        let body = r#"<button data-backend="x" data-loom-poly-action>Click</button>"#;
        let pairs = extract_button(body);
        assert!(pairs[0].2, "bare attribute counts as opt-out");
    }

    #[test]
    fn extract_button_pulls_label() {
        let body = r#"<button data-backend="foo">Click</button>"#;
        let pairs = extract_button(body);
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].0, "foo");
        assert_eq!(pairs[0].1, "Click");
    }
}
