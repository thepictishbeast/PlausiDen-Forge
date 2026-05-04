//! `html_semantic` — flag inline `style="..."` attributes.
//!
//! Per Loom doctrine: every visual rule lives in skin.css; HTML
//! is semantic markup. Inline style attrs are how layout drift
//! starts.
//!
//! Bash parity: `phase_html_semantic` in forge.sh — counts inline
//! `style="..."` per file.

use forge_core::{BuildCtx, BuildError, Finding, Phase};

use crate::html_walk::walk_html;

/// `html_semantic` phase implementation.
#[derive(Debug, Default)]
pub struct HtmlSemanticPhase;

impl Phase for HtmlSemanticPhase {
    fn name(&self) -> &'static str {
        "html_semantic"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let files = walk_html(&ctx.static_dir, self.name())?;
        let mut findings = Vec::new();

        for file in files {
            let count = count_inline_styles(&file.body);
            if count > 0 {
                findings.push(Finding::strict(
                    self.name(),
                    file.name.clone(),
                    format!(
                        "{count} inline style=\"...\" attribute(s); migrate to skin.css class"
                    ),
                ));
            }
        }

        Ok(findings)
    }
}

/// Count occurrences of `style="..."` (with at least one char
/// inside the quotes) in `body`.
///
/// BUG ASSUMPTION: this is a substring scan, not an HTML parse.
/// `data-style="foo"` matches if it ever appears literally — but
/// `data-style="foo"` is rare and the cost of a false positive is
/// "operator double-checks the file", which is correct behavior
/// for an audit tool. A real HTML parse is queued for forge-html.
fn count_inline_styles(body: &str) -> usize {
    let needle = "style=\"";
    let mut count = 0;
    let mut search = body;
    while let Some(idx) = search.find(needle) {
        let after = &search[idx + needle.len()..];
        // Require at least one non-quote character before closing.
        if let Some(end) = after.find('"') {
            if end > 0 {
                count += 1;
            }
            search = &after[end + 1..];
        } else {
            break;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_one_inline_style() {
        let body = r#"<div style="color: red">x</div>"#;
        assert_eq!(count_inline_styles(body), 1);
    }

    #[test]
    fn counts_multiple_inline_styles() {
        let body = r#"<div style="a: 1"><span style="b: 2">x</span></div>"#;
        assert_eq!(count_inline_styles(body), 2);
    }

    #[test]
    fn ignores_empty_style_attribute() {
        let body = r#"<div style="">x</div>"#;
        assert_eq!(count_inline_styles(body), 0);
    }

    #[test]
    fn ignores_pages_without_inline_style() {
        let body = "<div class=\"loom-card\"><p>safe</p></div>";
        assert_eq!(count_inline_styles(body), 0);
    }
}
