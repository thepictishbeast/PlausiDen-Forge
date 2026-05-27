//! `interaction_defaults` — substrate-level interaction defaults
//! with explicit-deviation discipline.
//!
//! Per task #388 (AI-agent-accessibility set #388-#391). The
//! substrate ships OPINIONATED defaults for every common
//! interactive surface (keyboard nav, focus management, ARIA
//! labelling, reduced-motion respect, form validation). Operators
//! (especially AI agents) MUST defer to defaults — silent
//! deviation is forbidden.
//!
//! "Accessibility" here = AI-agent-DX accessibility: a substrate
//! that's hard for agents to use produces inconsistent outputs.
//! By making the default path obvious + opinionated, the
//! substrate eliminates the per-session "what's the right way?"
//! research overhead.
//!
//! ## Rule
//!
//! For every interactive primitive (Button, Form, NavMenu,
//! Modal, etc.), the substrate ships one canonical default. To
//! deviate, the operator must:
//!
//! 1. Declare the deviation explicitly via a tagged config field
//!    (not by omission).
//! 2. Supply a `deviation_reason` string captured in the build
//!    record for audit.
//!
//! Operators that try to deviate silently (e.g., by setting
//! variants the substrate doesn't recognise, or by omitting
//! required accessibility fields) fail the build.

use serde::Serialize;

/// Interactive primitive family the substrate defaults apply to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum InteractiveFamily {
    /// Button / link CTAs.
    Button,
    /// Form fields + form submission.
    Form,
    /// Navigation menu (nav, mega_menu, vertical_nav, breadcrumb).
    Navigation,
    /// Modal / dialog / sheet / drawer.
    Modal,
    /// Tabs / accordions / disclosure widgets.
    Disclosure,
    /// Motion / animation primitives.
    Motion,
    /// Multi-step flow (auth, task-flow, checkout).
    Stepper,
}

impl InteractiveFamily {
    /// Stable slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Button => "button",
            Self::Form => "form",
            Self::Navigation => "navigation",
            Self::Modal => "modal",
            Self::Disclosure => "disclosure",
            Self::Motion => "motion",
            Self::Stepper => "stepper",
        }
    }
}

/// One canonical default the substrate ships.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct InteractionDefault {
    /// Family the default applies to.
    pub family: InteractiveFamily,
    /// Slug of the default behaviour.
    pub default_behaviour: &'static str,
    /// Rationale (often a past incident or accessibility
    /// requirement).
    pub rationale: &'static str,
    /// Accepted deviation slugs (closed set; anything else
    /// fails build).
    pub accepted_deviations: &'static [&'static str],
}

/// Canonical interaction-default registry.
pub const DEFAULTS: &[InteractionDefault] = &[
    InteractionDefault {
        family: InteractiveFamily::Button,
        default_behaviour: "keyboard_operable_focus_visible_aria_label",
        rationale: "Every button must be keyboard-operable, have a \
                    visible focus indicator, and carry an accessible \
                    name. Omitting any of these fails WCAG 2.1 AA \
                    + AI-DX consistency.",
        accepted_deviations: &[
            "decorative_only_aria_hidden", // e.g. icon-only decorative button paired with sr-only label
        ],
    },
    InteractionDefault {
        family: InteractiveFamily::Form,
        default_behaviour: "labels_for_every_field_inline_validation_error_summary",
        rationale: "Every form field needs an explicit label, \
                    inline validation on blur, and an error summary \
                    on submit failure. Without these, screen-reader \
                    users can't fill the form + AI agents can't \
                    reliably author one.",
        accepted_deviations: &[
            "no_inline_validation_minimal_form", // for single-field newsletter signup etc.
        ],
    },
    InteractionDefault {
        family: InteractiveFamily::Navigation,
        default_behaviour: "landmark_role_skip_link_aria_current",
        rationale: "Nav must carry role=navigation, every page must \
                    expose a skip-to-content link, and the current \
                    page must be marked via aria-current=page.",
        accepted_deviations: &[
            "single_page_no_skip_link",
        ],
    },
    InteractionDefault {
        family: InteractiveFamily::Modal,
        default_behaviour: "focus_trap_close_on_escape_aria_modal",
        rationale: "Modals must trap focus, close on Escape, and \
                    declare aria-modal=true with a labelled-by \
                    reference to the modal title.",
        accepted_deviations: &[], // No deviation: modals without focus trap are bugs.
    },
    InteractionDefault {
        family: InteractiveFamily::Disclosure,
        default_behaviour: "aria_expanded_toggleable_keyboard_enter_space",
        rationale: "Accordion / tab triggers must carry aria-expanded \
                    + respond to Enter and Space keys.",
        accepted_deviations: &[],
    },
    InteractionDefault {
        family: InteractiveFamily::Motion,
        default_behaviour: "respect_prefers_reduced_motion",
        rationale: "Every animation must check @media \
                    (prefers-reduced-motion: reduce) and disable / \
                    shorten itself when set. Vestibular disorders + \
                    AI-DX rendering predictability both require this.",
        accepted_deviations: &[
            "always_decorative_under_400ms", // very short non-essential motion
        ],
    },
    InteractionDefault {
        family: InteractiveFamily::Stepper,
        default_behaviour: "current_step_announced_back_button_aware",
        rationale: "Multi-step flows must announce the current step \
                    + support browser back-button navigation without \
                    losing prior-step state.",
        accepted_deviations: &[],
    },
];

/// Return all interaction defaults.
#[must_use]
pub fn all_defaults() -> &'static [InteractionDefault] {
    DEFAULTS
}

/// Look up the default for a given family.
#[must_use]
pub fn default_for(family: InteractiveFamily) -> Option<&'static InteractionDefault> {
    DEFAULTS.iter().find(|d| d.family == family)
}

/// Check whether a proposed deviation is accepted for a family.
/// Returns Some(reason) on acceptance, None on rejection.
#[must_use]
pub fn check_deviation(
    family: InteractiveFamily,
    deviation_slug: &str,
) -> bool {
    DEFAULTS
        .iter()
        .find(|d| d.family == family)
        .map(|d| d.accepted_deviations.contains(&deviation_slug))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_cover_every_family() {
        for family in [
            InteractiveFamily::Button,
            InteractiveFamily::Form,
            InteractiveFamily::Navigation,
            InteractiveFamily::Modal,
            InteractiveFamily::Disclosure,
            InteractiveFamily::Motion,
            InteractiveFamily::Stepper,
        ] {
            assert!(
                default_for(family).is_some(),
                "missing default for family {:?}",
                family
            );
        }
    }

    #[test]
    fn deviation_check_accepts_known() {
        assert!(check_deviation(
            InteractiveFamily::Button,
            "decorative_only_aria_hidden"
        ));
        assert!(check_deviation(
            InteractiveFamily::Motion,
            "always_decorative_under_400ms"
        ));
    }

    #[test]
    fn deviation_check_rejects_unknown() {
        assert!(!check_deviation(
            InteractiveFamily::Button,
            "made_up_deviation"
        ));
    }

    #[test]
    fn modal_has_no_accepted_deviations() {
        // Modals without focus-trap are bugs, not deviations.
        let modal = default_for(InteractiveFamily::Modal).unwrap();
        assert!(modal.accepted_deviations.is_empty());
    }

    #[test]
    fn family_slug_stable() {
        assert_eq!(InteractiveFamily::Button.slug(), "button");
        assert_eq!(InteractiveFamily::Form.slug(), "form");
        assert_eq!(InteractiveFamily::Navigation.slug(), "navigation");
        assert_eq!(InteractiveFamily::Modal.slug(), "modal");
        assert_eq!(InteractiveFamily::Disclosure.slug(), "disclosure");
        assert_eq!(InteractiveFamily::Motion.slug(), "motion");
        assert_eq!(InteractiveFamily::Stepper.slug(), "stepper");
    }

    #[test]
    fn every_rationale_non_empty() {
        for d in DEFAULTS {
            assert!(!d.rationale.is_empty());
        }
    }
}
