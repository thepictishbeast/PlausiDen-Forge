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
                findings.push(Finding::strict(
                    self.name(),
                    n.clone(),
                    "missing <main> landmark",
                ));
            }
            if !body.contains("<header") {
                findings.push(Finding::strict(
                    self.name(),
                    n.clone(),
                    "missing <header> landmark",
                ));
            }
            if !body.contains("<footer") {
                findings.push(Finding::strict(
                    self.name(),
                    n.clone(),
                    "missing <footer> landmark",
                ));
            }
            if !body.contains("<nav") {
                findings.push(Finding::warn(
                    self.name(),
                    n.clone(),
                    "missing <nav> landmark (acceptable on settings pages)",
                ));
            }
            if !body.contains(r#"class="loom-skip""#) {
                findings.push(Finding::warn(self.name(), n.clone(), "missing skip-link"));
            }
            if !body.contains("<html lang=") {
                findings.push(Finding::strict(
                    self.name(),
                    n.clone(),
                    "<html> missing lang attribute",
                ));
            }
        }

        Ok(findings)
    }
}
