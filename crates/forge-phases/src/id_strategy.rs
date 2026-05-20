//! `id_strategy` — enforce ID-attribute invariants.
//!
//! Bash parity: `phase_id_strategy`. Always-on checks:
//!
//! 1. No duplicate `id="X"` within a page (HTML spec).
//! 2. Every `<label for="X">` resolves to an `id="X"` on the page.
//! 3. Every `aria-labelledby` / `aria-describedby` / `aria-controls`
//!    / `aria-owns` token resolves to an id on the page.
//! 4. Every skip-link (`<a class="loom-skip ..." href="#X">`) points
//!    to an existing `id="X"`.

use std::collections::{BTreeSet, HashMap};

use forge_core::{BuildCtx, BuildError, Finding, Phase};

use crate::html_walk::walk_html;

/// `id_strategy` phase.
#[derive(Debug, Default)]
pub struct IdStrategyPhase;

impl Phase for IdStrategyPhase {
    fn name(&self) -> &'static str {
        "id_strategy"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let files = walk_html(&ctx.static_dir, self.name())?;
        let mut findings = Vec::new();

        for file in files {
            let body = file.body.as_str();
            let n = file.name.clone();

            // 1. Collect every id="X" + duplicates.
            let ids = collect_attr_values(body, "id=\"");
            let id_set: BTreeSet<&str> = ids.iter().map(|s| s.as_str()).collect();
            let mut counts: HashMap<&str, usize> = HashMap::new();
            for id in &ids {
                *counts.entry(id.as_str()).or_insert(0) += 1;
            }
            for (id, count) in counts {
                if count > 1 {
                    findings.push(Finding::strict(
                        self.name(),
                        n.clone(),
                        format!("duplicate id=\"{id}\" ({count} occurrences)"),
                    ));
                }
            }

            // 2. <label for="X"> resolves.
            for for_id in extract_label_for(body) {
                if !id_set.contains(for_id.as_str()) {
                    findings.push(Finding::strict(
                        self.name(),
                        n.clone(),
                        format!("<label for=\"{for_id}\"> has no matching id"),
                    ));
                }
            }

            // 3. aria-* references resolve.
            for (attr, token) in extract_aria_refs(body) {
                if !id_set.contains(token.as_str()) {
                    findings.push(Finding::strict(
                        self.name(),
                        n.clone(),
                        format!("{attr}=\"{token}\" has no matching id"),
                    ));
                }
            }

            // 4. Skip-link target resolves.
            for target in extract_skiplinks(body) {
                if !id_set.contains(target.as_str()) {
                    findings.push(Finding::strict(
                        self.name(),
                        n.clone(),
                        format!("skip-link href=\"#{target}\" has no matching id"),
                    ));
                }
            }
        }

        Ok(findings)
    }
}

/// Pull every `id="X"` (or other attribute prefix) value.
fn collect_attr_values(body: &str, prefix: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut search = body;
    while let Some(idx) = search.find(prefix) {
        let after = &search[idx + prefix.len()..];
        if let Some(end) = after.find('"') {
            // BUG ASSUMPTION: prefix matches as substring — must
            // verify left boundary so e.g. `data-id="..."` doesn't
            // match a search for `id="`. The boundary is the char
            // immediately before `prefix`.
            //
            // Issue #8 fix (2026-05-20): if the prior byte is >= 0x80
            // it's part of a multi-byte UTF-8 rune (leading or
            // continuation byte), which means the prior "character"
            // is a non-ASCII letter — treat as word continuation,
            // not a boundary. Previously, `as char` on a continuation
            // byte produced a U+0080-U+00BF char that failed both the
            // alphanumeric and dash/underscore checks, falsely
            // marking a UTF-8-internal position as a word boundary.
            let left_ok = if idx == 0 {
                true
            } else {
                let prev_byte = search.as_bytes()[idx - 1];
                if prev_byte >= 0x80 {
                    false
                } else {
                    let prev = prev_byte as char;
                    !prev.is_ascii_alphanumeric() && prev != '-' && prev != '_'
                }
            };
            if left_ok {
                out.push(after[..end].to_owned());
            }
            search = &after[end + 1..];
        } else {
            break;
        }
    }
    out
}

/// Extract every `<label for="X">` value.
fn extract_label_for(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let lower = body.to_ascii_lowercase();
    let mut search = lower.as_str();
    let mut offset = 0usize;
    while let Some(idx) = search.find("<label") {
        let after_open = &search[idx..];
        let Some(end) = after_open.find('>') else {
            break;
        };
        let abs_start = offset + idx;
        let tag_real = &body[abs_start..abs_start + end];
        if let Some(val) = extract_attr(tag_real, "for=\"") {
            out.push(val);
        }
        let advance = idx + end + 1;
        offset += advance;
        search = &search[advance..];
    }
    out
}

/// Extract every `aria-{labelledby|describedby|controls|owns}="..."`
/// value, splitting space-separated tokens. Returns `(attr, token)`
/// pairs for finding messages.
fn extract_aria_refs(body: &str) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    for attr in [
        "aria-labelledby",
        "aria-describedby",
        "aria-controls",
        "aria-owns",
    ] {
        let prefix = format!("{attr}=\"");
        let mut search = body;
        while let Some(idx) = search.find(&prefix) {
            let after = &search[idx + prefix.len()..];
            if let Some(end) = after.find('"') {
                for tok in after[..end].split_whitespace() {
                    out.push((attr, tok.to_owned()));
                }
                search = &after[end + 1..];
            } else {
                break;
            }
        }
    }
    out
}

/// Extract every `<a class="...loom-skip..." href="#X">` target.
fn extract_skiplinks(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let lower = body.to_ascii_lowercase();
    let mut search = lower.as_str();
    let mut offset = 0usize;
    while let Some(idx) = search.find("<a") {
        let after_open = &search[idx..];
        let Some(end) = after_open.find('>') else {
            break;
        };
        let abs_start = offset + idx;
        let tag_real = &body[abs_start..abs_start + end];
        let class_val = extract_attr(tag_real, "class=\"").unwrap_or_default();
        if class_val.split_whitespace().any(|c| c == "loom-skip") {
            if let Some(href) = extract_attr(tag_real, "href=\"") {
                if let Some(target) = href.strip_prefix('#') {
                    out.push(target.to_owned());
                }
            }
        }
        let advance = idx + end + 1;
        offset += advance;
        search = &search[advance..];
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
    fn collect_id_distinguishes_from_data_id() {
        let body = r#"<div id="a"></div><div data-id="b"></div>"#;
        let v = collect_attr_values(body, "id=\"");
        assert_eq!(v, vec!["a".to_owned()]);
    }

    #[test]
    fn extract_label_for_basic() {
        let body = r#"<label for="email">Email</label>"#;
        let v = extract_label_for(body);
        assert_eq!(v, vec!["email".to_owned()]);
    }

    #[test]
    fn extract_aria_refs_splits_tokens() {
        let body = r#"<p aria-labelledby="t1 t2 t3">x</p>"#;
        let refs = extract_aria_refs(body);
        let tokens: Vec<&str> = refs.iter().map(|(_, t)| t.as_str()).collect();
        assert_eq!(tokens, vec!["t1", "t2", "t3"]);
    }

    #[test]
    fn extract_skiplinks_basic() {
        // Double-hash raw string: body contains `"#main"`.
        let body = r##"<a class="loom-skip" href="#main">Skip</a>"##;
        let v = extract_skiplinks(body);
        assert_eq!(v, vec!["main".to_owned()]);
    }

    #[test]
    fn extract_skiplinks_ignores_non_skip_anchors() {
        let body = r##"<a href="#section">jump</a>"##;
        let v = extract_skiplinks(body);
        assert!(v.is_empty());
    }
}
