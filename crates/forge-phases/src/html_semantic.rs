//! `html_semantic` — flag inline `style="..."` attributes AND
//! `<div role="<landmark>">` redundancies (use the semantic
//! element instead).
//!
//! Per Loom doctrine + user directive 2026-05-13: every visual
//! rule lives in skin.css; HTML is semantic markup. Inline style
//! attrs are how layout drift starts. Likewise, a `<div>` with an
//! ARIA landmark role is a missed opportunity for the equivalent
//! semantic element — `<div role="banner">` should be `<header>`,
//! `<div role="main">` should be `<main>`, etc. axe-core flags
//! these (rule `landmark-no-duplicate-banner`, `region` family);
//! catch them at build time, not in the audit.
//!
//! Bash parity: `phase_html_semantic` (style-attr count). The
//! div-role check is a Rust-only addition (T67, supersociety
//! supplement).

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
                findings.push(
                    Finding::strict(
                        self.name(),
                        file.name.clone(),
                        format!("{count} inline style=\"...\" attribute(s)"),
                    )
                    .citing(["prim-006", "prim-007"])
                    .why("inline styles bypass the typed loom-tokens theme system; they don't respect dark mode, AMOLED, prefers-reduced-motion, or RTL — and they can't be CSP-sandboxed cleanly")
                    .fix("the Loom primitive that emitted this HTML needs a variant or skin.css class that captures the styling intent; remove the style= attribute from the primitive's render emission")
                    .skill("add-loom-primitive")
                    .avoid("don't sed/grep style= attributes out of static/ — the build re-emits them; fix the source in loom-cms-render"),
                );
            }
            // T67: <div role="<landmark>"> → use the semantic
            // element. One finding per matched role per file
            // (de-duped) so the report stays readable on a
            // page that repeats the mistake many times.
            for hit in find_div_landmark_roles(&file.body) {
                findings.push(
                    Finding::strict(
                        self.name(),
                        file.name.clone(),
                        format!(
                            "<div role=\"{role}\"> ({count}× in this file)",
                            role = hit.role,
                            count = hit.count,
                        ),
                    )
                    .citing(["a11y-001"])
                    .why("screen readers treat semantic elements (<nav>, <main>, <header>, <footer>, <article>) and ARIA-role divs differently; the semantic element is more reliably announced")
                    .fix(format!(
                        "in the Loom primitive that emits this element, replace `<div role=\"{}\">` with `<{}>` — same semantics, better SR support, fewer bytes",
                        hit.role, hit.suggested_element
                    ))
                    .skill("add-loom-primitive"),
                );
            }
        }

        Ok(findings)
    }
}

/// One `<div role="X">` finding per file. `role` and
/// `suggested_element` are static slices because both come
/// from a closed enum.
#[derive(Debug, PartialEq, Eq)]
struct DivLandmarkHit {
    role: &'static str,
    suggested_element: &'static str,
    count: usize,
}

/// Pairs of (ARIA landmark role, semantic-HTML replacement).
///
/// `region` deliberately omitted — `<section aria-labelledby>` is
/// ARIA's own recommended pattern and the only landmark that
/// requires `role` on the element today.
const LANDMARK_ROLES: &[(&str, &str)] = &[
    ("banner", "header"),
    ("main", "main"),
    ("contentinfo", "footer"),
    ("navigation", "nav"),
    ("complementary", "aside"),
    ("search", "search"),
    ("form", "form"),
];

/// Walk `body` for `<div ... role="X" ...>` where X is one of the
/// landmark roles above. Returns one `DivLandmarkHit` per role
/// matched (de-duped + counted).
///
/// BUG ASSUMPTION: this is a substring scan, not an HTML parse.
/// A real HTML parse is queued for forge-html. False positives
/// surface in attribute values literally containing
/// `<div role="banner"`, which we never emit.
fn find_div_landmark_roles(body: &str) -> Vec<DivLandmarkHit> {
    let mut out: Vec<DivLandmarkHit> = Vec::new();
    for &(role, element) in LANDMARK_ROLES {
        let count = count_div_with_role(body, role);
        if count > 0 {
            out.push(DivLandmarkHit {
                role,
                suggested_element: element,
                count,
            });
        }
    }
    out
}

/// Count `<div ... role="<role>" ...>` occurrences. Tolerant of
/// any attribute order or whitespace inside the open tag.
fn count_div_with_role(body: &str, role: &str) -> usize {
    let needle = format!(r#"role="{role}""#);
    let mut count = 0;
    let mut search = body;
    while let Some(idx) = search.find(needle.as_str()) {
        // Walk backwards from `idx` to the nearest '<'. If that
        // tag opens with `<div` (case-insensitive ASCII), count it.
        let prefix = &search[..idx];
        if let Some(lt) = prefix.rfind('<') {
            let tag_start = &prefix[lt..];
            // Check whether the tag is `<div` followed by a
            // word-boundary char (space, `>`, `/`, tab, newline).
            let after_div = tag_start.get(0..4).map(str::to_ascii_lowercase);
            if after_div.as_deref() == Some("<div")
                && tag_start
                    .as_bytes()
                    .get(4)
                    .copied()
                    .is_some_and(|b| matches!(b, b' ' | b'>' | b'/' | b'\t' | b'\n' | b'\r'))
            {
                count += 1;
            }
        }
        search = &search[idx + needle.len()..];
    }
    count
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

    // ---- T67: <div role="<landmark>"> detection ----

    #[test]
    fn flags_div_role_banner() {
        let body = r#"<div role="banner" class="hdr">…</div>"#;
        let hits = find_div_landmark_roles(body);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].role, "banner");
        assert_eq!(hits[0].suggested_element, "header");
        assert_eq!(hits[0].count, 1);
    }

    #[test]
    fn flags_all_landmark_roles_on_div() {
        let body = r#"
          <div role="banner">h</div>
          <div role="main">m</div>
          <div role="contentinfo">f</div>
          <div role="navigation">n</div>
          <div role="complementary">a</div>
          <div role="search">s</div>
          <div role="form">f</div>
        "#;
        let hits = find_div_landmark_roles(body);
        let roles: Vec<_> = hits.iter().map(|h| h.role).collect();
        for expected in [
            "banner",
            "main",
            "contentinfo",
            "navigation",
            "complementary",
            "search",
            "form",
        ] {
            assert!(
                roles.contains(&expected),
                "missing role={expected} in {roles:?}"
            );
        }
    }

    #[test]
    fn dedupes_repeated_role_per_file() {
        // Multiple matches of the same role collapse into one
        // finding with `count` populated — keeps reports readable.
        let body = r#"
          <div role="banner">a</div>
          <div role="banner">b</div>
          <div role="banner">c</div>
        "#;
        let hits = find_div_landmark_roles(body);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].count, 3);
    }

    #[test]
    fn does_not_flag_role_on_semantic_element() {
        // <header role="banner"> is redundant but not "wrong" the
        // same way <div role="banner"> is — axe still flags it
        // separately. T67 v1 only chases the div pattern.
        let body = r#"<header role="banner">x</header>"#;
        assert!(find_div_landmark_roles(body).is_empty());
    }

    #[test]
    fn does_not_flag_role_on_section_or_article() {
        let body = r#"
          <section role="region" aria-labelledby="x">a</section>
          <article role="article">b</article>
        "#;
        assert!(find_div_landmark_roles(body).is_empty());
    }

    #[test]
    fn ignores_div_with_non_landmark_role() {
        // role="button", role="dialog" etc. are NOT landmarks
        // and don't have a one-shot semantic replacement.
        let body = r#"
          <div role="button">x</div>
          <div role="dialog">y</div>
          <div role="region" aria-labelledby="lbl">z</div>
        "#;
        assert!(find_div_landmark_roles(body).is_empty());
    }

    #[test]
    fn case_insensitive_div_match() {
        // HTML tags are case-insensitive; the check should be too.
        let body = r#"<DIV role="banner">x</DIV>"#;
        let hits = find_div_landmark_roles(body);
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn does_not_match_word_boundary_violation() {
        // <divisor> shouldn't match <div ; the word-boundary
        // check after `<div` prevents this.
        let body = r#"<divisor role="banner">x</divisor>"#;
        assert!(find_div_landmark_roles(body).is_empty());
    }

    #[test]
    fn handles_other_attrs_before_role() {
        let body = r#"<div class="a" id="b" role="main" data-x>x</div>"#;
        let hits = find_div_landmark_roles(body);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].role, "main");
    }

    #[test]
    fn flags_phase_finding_emitted_per_role() {
        use forge_core::{BuildCtx, Phase};
        // Build a fake static dir containing one HTML file with a
        // div-role-banner and a div-role-main.
        let tmp = std::env::temp_dir().join(format!(
            "html-sem-t67-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(tmp.join("static")).expect("mk static");
        std::fs::write(
            tmp.join("static/index.html"),
            r#"<!doctype html><html><body>
            <div role="banner">h</div>
            <div role="main">m</div>
            </body></html>"#,
        )
        .expect("write html");
        let ctx = BuildCtx {
            root: tmp.clone(),
            static_dir: tmp.join("static"),
            mode: forge_core::BuildMode::Poc,
        };
        let findings = HtmlSemanticPhase.run(&ctx).expect("run");
        // Per task #201: message identifies the role; the recommended
        // replacement element lives in advocacy.substrate_fix.
        let role_and_fix: Vec<(String, String)> = findings
            .iter()
            .map(|f| (f.message.clone(), f.advocacy.substrate_fix.clone()))
            .collect();
        assert!(
            role_and_fix
                .iter()
                .any(|(m, fix)| m.contains("banner") && fix.contains("<header>")),
            "missing banner→header finding (advocacy.fix should suggest <header>): {role_and_fix:?}"
        );
        assert!(
            role_and_fix
                .iter()
                .any(|(m, fix)| m.contains("\"main\"") && fix.contains("<main>")),
            "missing main→main finding (advocacy.fix should suggest <main>): {role_and_fix:?}"
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
