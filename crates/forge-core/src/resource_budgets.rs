//! `resource_budgets` — per-PageKind budgets for generation
//! resources (primitives, colors, fonts, images, etc.) +
//! budget-check enforcement.
//!
//! Per task #381. Without budgets, generation maximalism
//! produces 8-section pages with 4 themes and 12 fonts. With
//! per-PageKind budgets, a brief-kind page is forbidden from
//! using 4 distinct fonts; a marketing-landing page is allowed
//! more colors but capped on animation density.
//!
//! ## Why per-PageKind
//!
//! The substrate-reframe doctrine #360 says defaults must be
//! PageKind-driven. Same logic applies to BUDGETS: a kinfolk-
//! shape editorial site is *expected* to use 1-2 photos as
//! decorative dividers; a brief-shape site capping at 0 photos
//! is the right discipline. One global budget number across
//! all PageKinds would either over-budget the brief case or
//! under-budget the editorial case.

use serde::Serialize;

/// Kind of resource being budgeted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BudgetKind {
    /// Distinct CmsSection variants used.
    PrimitiveKinds,
    /// Total CmsSection occurrences per page.
    PrimitivesPerPage,
    /// Distinct theme tokens (light/dark/warm/editorial).
    Themes,
    /// Distinct font families.
    Fonts,
    /// Distinct colors beyond the theme baseline.
    Colors,
    /// Image / picture / image_grid count per page.
    Images,
    /// Animation / motion sections (marquee/reveal/etc.).
    Animations,
    /// Total prose character count per page (substance floor).
    ProseChars,
}

impl BudgetKind {
    /// Stable slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::PrimitiveKinds => "primitive_kinds",
            Self::PrimitivesPerPage => "primitives_per_page",
            Self::Themes => "themes",
            Self::Fonts => "fonts",
            Self::Colors => "colors",
            Self::Images => "images",
            Self::Animations => "animations",
            Self::ProseChars => "prose_chars",
        }
    }
}

/// One budget entry.
#[derive(Debug, Clone, Copy, Serialize)]
#[non_exhaustive]
pub struct Budget {
    /// What is being budgeted.
    pub kind: BudgetKind,
    /// Hard maximum. Crossing → Block-severity finding.
    pub max: u32,
    /// Soft warning threshold. Crossing → Warn-severity.
    pub soft_warn_at: u32,
    /// Minimum (for ProseChars-style floor budgets).
    /// Below → Block.
    pub min: u32,
}

/// A complete budget set for a specific PageKind.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct BudgetSet {
    /// PageKind slug this set applies to.
    pub page_kind: &'static str,
    /// All budgets in the set.
    pub budgets: Vec<Budget>,
}

/// Per-PageKind canonical budget sets. Hand-curated; tuned
/// against the 5-site reference frame.
#[must_use]
pub fn budgets_for(page_kind: &str) -> BudgetSet {
    let budgets = match page_kind {
        "marketing_landing" => vec![
            Budget { kind: BudgetKind::PrimitiveKinds, max: 12, soft_warn_at: 9, min: 4 },
            Budget { kind: BudgetKind::PrimitivesPerPage, max: 12, soft_warn_at: 9, min: 4 },
            Budget { kind: BudgetKind::Themes, max: 1, soft_warn_at: 1, min: 1 },
            Budget { kind: BudgetKind::Fonts, max: 3, soft_warn_at: 2, min: 1 },
            Budget { kind: BudgetKind::Colors, max: 6, soft_warn_at: 4, min: 2 },
            Budget { kind: BudgetKind::Images, max: 8, soft_warn_at: 6, min: 0 },
            Budget { kind: BudgetKind::Animations, max: 3, soft_warn_at: 2, min: 0 },
            Budget { kind: BudgetKind::ProseChars, max: 5000, soft_warn_at: 4000, min: 600 },
        ],
        "brief" => vec![
            Budget { kind: BudgetKind::PrimitiveKinds, max: 4, soft_warn_at: 3, min: 1 },
            Budget { kind: BudgetKind::PrimitivesPerPage, max: 6, soft_warn_at: 4, min: 1 },
            Budget { kind: BudgetKind::Themes, max: 1, soft_warn_at: 1, min: 1 },
            Budget { kind: BudgetKind::Fonts, max: 2, soft_warn_at: 1, min: 1 },
            Budget { kind: BudgetKind::Colors, max: 3, soft_warn_at: 2, min: 1 },
            Budget { kind: BudgetKind::Images, max: 1, soft_warn_at: 0, min: 0 },
            Budget { kind: BudgetKind::Animations, max: 0, soft_warn_at: 0, min: 0 },
            Budget { kind: BudgetKind::ProseChars, max: 20000, soft_warn_at: 15000, min: 400 },
        ],
        "editorial" => vec![
            Budget { kind: BudgetKind::PrimitiveKinds, max: 10, soft_warn_at: 7, min: 3 },
            Budget { kind: BudgetKind::PrimitivesPerPage, max: 14, soft_warn_at: 10, min: 4 },
            Budget { kind: BudgetKind::Themes, max: 1, soft_warn_at: 1, min: 1 },
            Budget { kind: BudgetKind::Fonts, max: 3, soft_warn_at: 2, min: 1 },
            Budget { kind: BudgetKind::Colors, max: 5, soft_warn_at: 3, min: 2 },
            Budget { kind: BudgetKind::Images, max: 12, soft_warn_at: 8, min: 1 },
            Budget { kind: BudgetKind::Animations, max: 2, soft_warn_at: 1, min: 0 },
            Budget { kind: BudgetKind::ProseChars, max: 15000, soft_warn_at: 12000, min: 800 },
        ],
        "civic" => vec![
            Budget { kind: BudgetKind::PrimitiveKinds, max: 8, soft_warn_at: 6, min: 3 },
            Budget { kind: BudgetKind::PrimitivesPerPage, max: 10, soft_warn_at: 7, min: 3 },
            Budget { kind: BudgetKind::Themes, max: 1, soft_warn_at: 1, min: 1 },
            Budget { kind: BudgetKind::Fonts, max: 2, soft_warn_at: 1, min: 1 },
            Budget { kind: BudgetKind::Colors, max: 4, soft_warn_at: 3, min: 2 },
            Budget { kind: BudgetKind::Images, max: 3, soft_warn_at: 2, min: 0 },
            Budget { kind: BudgetKind::Animations, max: 0, soft_warn_at: 0, min: 0 },
            Budget { kind: BudgetKind::ProseChars, max: 8000, soft_warn_at: 6000, min: 500 },
        ],
        "documentation" => vec![
            Budget { kind: BudgetKind::PrimitiveKinds, max: 8, soft_warn_at: 6, min: 3 },
            Budget { kind: BudgetKind::PrimitivesPerPage, max: 30, soft_warn_at: 20, min: 4 },
            Budget { kind: BudgetKind::Themes, max: 2, soft_warn_at: 2, min: 1 },
            Budget { kind: BudgetKind::Fonts, max: 2, soft_warn_at: 2, min: 1 },
            Budget { kind: BudgetKind::Colors, max: 5, soft_warn_at: 4, min: 2 },
            Budget { kind: BudgetKind::Images, max: 6, soft_warn_at: 4, min: 0 },
            Budget { kind: BudgetKind::Animations, max: 1, soft_warn_at: 0, min: 0 },
            Budget { kind: BudgetKind::ProseChars, max: 50000, soft_warn_at: 40000, min: 1000 },
        ],
        // Fallback: marketing-landing budgets.
        _ => vec![
            Budget { kind: BudgetKind::PrimitiveKinds, max: 12, soft_warn_at: 9, min: 4 },
            Budget { kind: BudgetKind::PrimitivesPerPage, max: 12, soft_warn_at: 9, min: 4 },
            Budget { kind: BudgetKind::Themes, max: 1, soft_warn_at: 1, min: 1 },
            Budget { kind: BudgetKind::Fonts, max: 3, soft_warn_at: 2, min: 1 },
            Budget { kind: BudgetKind::Colors, max: 6, soft_warn_at: 4, min: 2 },
            Budget { kind: BudgetKind::Images, max: 8, soft_warn_at: 6, min: 0 },
            Budget { kind: BudgetKind::Animations, max: 3, soft_warn_at: 2, min: 0 },
            Budget { kind: BudgetKind::ProseChars, max: 5000, soft_warn_at: 4000, min: 600 },
        ],
    };

    BudgetSet {
        // Returning &'static str without leaking heap memory:
        // accept the slight waste of matching twice.
        page_kind: match page_kind {
            "marketing_landing" => "marketing_landing",
            "brief" => "brief",
            "editorial" => "editorial",
            "civic" => "civic",
            "documentation" => "documentation",
            _ => "fallback_marketing_landing",
        },
        budgets,
    }
}

/// Severity of a budget violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ViolationSeverity {
    /// At soft-warn threshold; ship allowed.
    Warn,
    /// Over hard max OR under hard min; block ship.
    Block,
}

/// One budget violation.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct BudgetViolation {
    /// Which budget was violated.
    pub kind: BudgetKind,
    /// Observed value that triggered the violation.
    pub observed: u32,
    /// Severity.
    pub severity: ViolationSeverity,
    /// Direction: "over" max or "under" min.
    pub direction: &'static str,
    /// The budget entry that fired.
    pub budget: Budget,
}

/// Compare observed values against a budget set; return every
/// violation found, sorted block-first.
#[must_use]
pub fn check_budgets(
    budgets: &BudgetSet,
    observed: &[(BudgetKind, u32)],
) -> Vec<BudgetViolation> {
    let mut out = Vec::new();
    for b in &budgets.budgets {
        let Some(&(_, value)) = observed.iter().find(|(k, _)| *k == b.kind) else {
            continue;
        };
        if value > b.max {
            out.push(BudgetViolation {
                kind: b.kind,
                observed: value,
                severity: ViolationSeverity::Block,
                direction: "over",
                budget: *b,
            });
        } else if value > b.soft_warn_at {
            out.push(BudgetViolation {
                kind: b.kind,
                observed: value,
                severity: ViolationSeverity::Warn,
                direction: "over",
                budget: *b,
            });
        } else if b.min > 0 && value < b.min {
            out.push(BudgetViolation {
                kind: b.kind,
                observed: value,
                severity: ViolationSeverity::Block,
                direction: "under",
                budget: *b,
            });
        }
    }
    out.sort_by(|a, b| {
        let sev = |s: ViolationSeverity| match s {
            ViolationSeverity::Block => 0,
            ViolationSeverity::Warn => 1,
        };
        sev(a.severity).cmp(&sev(b.severity))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budgets_for_known_kinds() {
        let mk = budgets_for("marketing_landing");
        assert_eq!(mk.page_kind, "marketing_landing");
        assert!(!mk.budgets.is_empty());

        let brief = budgets_for("brief");
        // Brief allows fewer fonts than marketing.
        let brief_fonts = brief
            .budgets
            .iter()
            .find(|b| b.kind == BudgetKind::Fonts)
            .map(|b| b.max)
            .unwrap();
        let mk_fonts = mk
            .budgets
            .iter()
            .find(|b| b.kind == BudgetKind::Fonts)
            .map(|b| b.max)
            .unwrap();
        assert!(brief_fonts <= mk_fonts);
    }

    #[test]
    fn budgets_for_unknown_falls_back() {
        let bs = budgets_for("unknown_kind");
        assert!(bs.page_kind.contains("fallback"));
    }

    #[test]
    fn check_within_budget_no_violations() {
        let bs = budgets_for("marketing_landing");
        let observed = vec![
            (BudgetKind::PrimitivesPerPage, 6),
            (BudgetKind::Fonts, 2),
            (BudgetKind::ProseChars, 2500),
        ];
        let violations = check_budgets(&bs, &observed);
        assert!(violations.is_empty());
    }

    #[test]
    fn check_over_max_blocks() {
        let bs = budgets_for("brief");
        let observed = vec![(BudgetKind::Images, 5)];
        let violations = check_budgets(&bs, &observed);
        assert!(!violations.is_empty());
        assert_eq!(violations[0].severity, ViolationSeverity::Block);
        assert_eq!(violations[0].direction, "over");
    }

    #[test]
    fn check_under_min_blocks() {
        let bs = budgets_for("marketing_landing");
        let observed = vec![(BudgetKind::ProseChars, 100)];
        let violations = check_budgets(&bs, &observed);
        assert_eq!(violations[0].severity, ViolationSeverity::Block);
        assert_eq!(violations[0].direction, "under");
    }

    #[test]
    fn check_soft_warn_returns_warn() {
        let bs = budgets_for("marketing_landing");
        // max=12, soft_warn=9 — observed 10 → Warn.
        let observed = vec![(BudgetKind::PrimitivesPerPage, 10)];
        let violations = check_budgets(&bs, &observed);
        assert_eq!(violations[0].severity, ViolationSeverity::Warn);
    }

    #[test]
    fn block_sorted_before_warn() {
        let bs = budgets_for("brief");
        let observed = vec![
            (BudgetKind::PrimitivesPerPage, 5), // 5 > 4 = soft_warn → Warn
            (BudgetKind::Animations, 3),         // 3 > 0 = max → Block
        ];
        let violations = check_budgets(&bs, &observed);
        assert_eq!(violations[0].severity, ViolationSeverity::Block);
    }

    #[test]
    fn budget_kind_slug_stable() {
        assert_eq!(BudgetKind::PrimitiveKinds.slug(), "primitive_kinds");
        assert_eq!(BudgetKind::PrimitivesPerPage.slug(), "primitives_per_page");
        assert_eq!(BudgetKind::Themes.slug(), "themes");
        assert_eq!(BudgetKind::Fonts.slug(), "fonts");
        assert_eq!(BudgetKind::Colors.slug(), "colors");
        assert_eq!(BudgetKind::Images.slug(), "images");
        assert_eq!(BudgetKind::Animations.slug(), "animations");
        assert_eq!(BudgetKind::ProseChars.slug(), "prose_chars");
    }
}
