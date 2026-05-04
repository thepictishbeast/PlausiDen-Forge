//! `label_consistency` — every (kind, key) pair (where kind is
//! `a` keyed on `href` or `button` keyed on `data-backend`) must
//! carry exactly one distinct visible label across the whole site.
//!
//! Bash parity: `phase_label_consistency`. Owner-surfaced bug it
//! catches: nav link said "Post a Skill" while CTA button said
//! "Post skill" — same destination, two labels.

use std::collections::BTreeMap;

use forge_core::{BuildCtx, BuildError, Finding, Phase};

use crate::html_walk::walk_html;

/// `label_consistency` phase.
#[derive(Debug, Default)]
pub struct LabelConsistencyPhase;

impl Phase for LabelConsistencyPhase {
    fn name(&self) -> &'static str {
        "label_consistency"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let files = walk_html(&ctx.static_dir, self.name())?;
        // (kind, key) -> {label: [files]}
        let mut groups: BTreeMap<(&'static str, String), BTreeMap<String, Vec<String>>> =
            BTreeMap::new();

        for file in &files {
            for (href, inner) in extract_anchor(&file.body) {
                let label = normalize_label(&inner);
                if label.is_empty() || label.starts_with('▶') {
                    continue;
                }
                groups
                    .entry(("a", href))
                    .or_default()
                    .entry(label)
                    .or_default()
                    .push(file.name.clone());
            }
            for (backend, inner) in extract_button(&file.body) {
                let label = normalize_label(&inner);
                if label.is_empty() {
                    continue;
                }
                groups
                    .entry(("button", backend))
                    .or_default()
                    .entry(label)
                    .or_default()
                    .push(file.name.clone());
            }
        }

        let mut findings = Vec::new();
        for ((kind, key), label_map) in &groups {
            if label_map.len() > 1 {
                let summary: Vec<String> = label_map
                    .iter()
                    .map(|(label, files)| format!("\"{label}\" ({}x)", files.len()))
                    .collect();
                // T11.3.2 / 2026-05-04: severity is Warn pending the
                // polymorphic-action opt-out design (T513). The
                // current detector flags every (kind, key) with
                // multiple labels — but legitimate polymorphism
                // exists (e.g. one data-backend="list-challenges"
                // wired to many filter buttons whose labels are
                // the filter modifier, not the action). Once the
                // design system declares an opt-out attribute
                // (e.g. data-loom-poly-action), this becomes
                // Strict on the residual non-opted-out cases.
                findings.push(Finding::warn(
                    self.name(),
                    "static/",
                    format!(
                        "{kind}[{key}] — {} distinct labels: {} — declare data-loom-poly-action if intentional polymorphism (T513)",
                        label_map.len(),
                        summary.join(", ")
                    ),
                ));
            }
        }
        Ok(findings)
    }
}

/// Extract every `<a href="..."> INNER </a>` as (href, inner_html).
fn extract_anchor(body: &str) -> Vec<(String, String)> {
    extract_pair(body, "<a", "href=\"", "</a>")
}

/// Extract every `<button data-backend="..."> INNER </button>`.
fn extract_button(body: &str) -> Vec<(String, String)> {
    extract_pair(body, "<button", "data-backend=\"", "</button>")
}

fn extract_pair(
    body: &str,
    open_tag: &str,
    attr_prefix: &str,
    close_tag: &str,
) -> Vec<(String, String)> {
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
        let Some(close_idx) = after_open.find(close_tag) else {
            break;
        };
        let inner = &after_open[..close_idx];
        out.push((key, inner.to_owned()));
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
    // Cheap entity decode (only common ones).
    let decoded = out
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");
    // Collapse whitespace.
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
    fn extract_anchor_pulls_label() {
        let body = r#"<a href="/x.html">Foo</a><a href="/y.html"><span>Bar</span></a>"#;
        let pairs = extract_anchor(body);
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], ("/x.html".to_owned(), "Foo".to_owned()));
    }

    #[test]
    fn extract_button_pulls_label() {
        let body = r#"<button data-backend="foo">Click</button>"#;
        let pairs = extract_button(body);
        assert_eq!(pairs, vec![("foo".to_owned(), "Click".to_owned())]);
    }
}
