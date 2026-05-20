//! `a11y_landmarks` — every shipped HTML page must carry the full
//! landmark set: `<header>`, `<main>`, `<footer>`, plus a skip-link
//! and `<html lang=...>`. `<nav>` is warn-level (acceptable to omit
//! on dedicated settings pages).
//!
//! Bash parity: `phase_a11y_landmarks`.

use forge_core::{BuildCtx, BuildError, Finding, Phase};

use crate::html_walk::walk_html;

/// `a11y_landmarks` phase.
#[derive(Debug, Default)]
pub struct A11yLandmarksPhase;

impl Phase for A11yLandmarksPhase {
    fn name(&self) -> &'static str {
        "a11y_landmarks"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let files = walk_html(&ctx.static_dir, self.name())?;
        let mut findings = Vec::new();

        for file in files {
            let body = file.body.as_str();
            let n = file.name.clone();

            if !body.contains("<main") {
                findings.push(
                    Finding::strict(self.name(), n.clone(), "missing <main> landmark")
                        .citing(["a11y-001"])
                        .why("screen readers rely on `<main>` to skip past nav into primary content; without it, keyboard / SR users repeatedly tab through navigation on every page")
                        .fix("page-shell template should wrap the rendered page body in `<main>`. Add the landmark to the Loom page-shell primitive — NOT to the rendered static/ HTML")
                        .skill("add-loom-primitive"),
                );
            }
            if !body.contains("<header") {
                findings.push(
                    Finding::strict(self.name(), n.clone(), "missing <header> landmark")
                        .citing(["a11y-001"])
                        .why("`<header>` is the canonical SR landmark for site identity + global nav; absence forces users to scan for branding")
                        .fix("emit `<header>` in the Loom page-shell template surrounding nav + brand area")
                        .skill("add-loom-primitive"),
                );
            }
            if !body.contains("<footer") {
                findings.push(
                    Finding::strict(self.name(), n.clone(), "missing <footer> landmark")
                        .citing(["a11y-001"])
                        .why("`<footer>` is the canonical SR landmark for legal + secondary links; absence loses an SR jump target")
                        .fix("emit `<footer>` in the Loom page-shell template — the footer schema in cms/<page>.json should already populate it")
                        .skill("add-loom-primitive"),
                );
            }
            if !body.contains("<nav") {
                findings.push(
                    Finding::warn(
                        self.name(),
                        n.clone(),
                        "missing <nav> landmark (acceptable on settings pages)",
                    )
                    .citing(["a11y-001"])
                    .why("`<nav>` is the canonical SR landmark for navigation; this page omits it (acceptable on standalone settings pages but flagged for review)")
                    .fix("if this page has navigation, emit `<nav>` in the Loom page-shell template; if intentional (settings), document in the CMS page schema"),
                );
            }
            if !body.contains(r#"class="loom-skip""#) {
                findings.push(
                    Finding::warn(self.name(), n.clone(), "missing skip-link")
                        .citing(["a11y-001"])
                        .why("keyboard users need a `Skip to content` link as the first focusable element; absence means tabbing through entire nav on every page")
                        .fix("the Loom page-shell template should emit the `loom-skip` class skip-link as its first focusable element")
                        .skill("add-loom-primitive"),
                );
            }
            if !body.contains("<html lang=") {
                findings.push(
                    Finding::strict(self.name(), n.clone(), "<html> missing lang attribute")
                        .citing(["a11y-001"])
                        .why("without `lang`, screen readers cannot pick the correct pronunciation engine; the page sounds wrong in any language")
                        .fix("the Loom page-shell `<html>` emission must include `lang=\"<bcp-47-tag>\"` (e.g. lang=\"en\"). Wire the CMS page's declared locale through to the shell"),
                );
            }
        }

        Ok(findings)
    }
}
