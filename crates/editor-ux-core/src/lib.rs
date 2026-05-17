//! `editor-ux-core` — typed editor-UX state.
//!
//! Per `PLATFORM_ROADMAP.md` §8 + `feedback_forge_default_themes_a11y`:
//! the editor surface (Loom `edit-serve`, CMS admin, future
//! per-tenant Claude Code chat) is **canvas-dominant**. Reader
//! sees the rendered page at all times; chrome adapts around it
//! rather than around itself.
//!
//! ### Four UX primitives shipped here
//!
//! 1. [`ZoomLevel`] — three discrete zooms (Overview / Page /
//!    Detail) the editor cycles between. Closed enum, not a
//!    continuous scale; gives the consumer three predictable
//!    layouts rather than 1000 jittery ones.
//!
//! 2. [`InspectorMode`] — the inspector panel morphs (Hidden /
//!    Minimal / Full / Floating). State is typed + transitions
//!    are predictable.
//!
//! 3. [`PaletteState`] — the ⌘K (or Ctrl+K) palette. Open / closed
//!    + current query + mode discriminator (Commands / Search /
//!    Navigation / AI).
//!
//! 4. [`AiBarState`] — the AI assistance bar. Off / Suggestions /
//!    Drafting / Reviewing. Closed enum so consumers exhaustively
//!    handle each lifecycle state.
//!
//! [`EditorState`] bundles the four. Designed for serialization
//! across the SSR/CSR boundary so an SSR-rendered editor can pick
//! up where the CSR session left off.
//!
//! ### What this commit does NOT ship
//!
//! No JS, no HTML, no canvas implementation. This crate is the
//! typed state contract every implementation projects through.
//! The actual editor lives in `loom-edit-serve` (Rust SSR) +
//! optionally a Web Components layer (a follow-up). Consumers
//! integrate against the types here so the editor + admin UI +
//! future native shells all share one state model.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::{Deserialize, Serialize};

/// Three discrete zoom levels the canvas-dominant editor cycles
/// between.
///
/// Discrete (not continuous) so the editor renders three
/// predictable layouts. A free continuous scale gives 1000 buggy
/// breakpoints; three named tiers gives three reviewable ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ZoomLevel {
    /// Multi-page overview (cards in a grid). Used to navigate
    /// between pages without entering edit mode.
    Overview,
    /// Single-page view at 1:1 — the default editing zoom.
    #[default]
    Page,
    /// Zoomed-in detail view (e.g. typography micro-editing,
    /// pixel-level alignment). Hides most chrome.
    Detail,
}

impl ZoomLevel {
    /// Stable kebab-case slug for serialization + UI markers.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Page => "page",
            Self::Detail => "detail",
        }
    }

    /// CSS scale value that pairs with the zoom (the editor
    /// applies `transform: scale(N)` on the canvas-root element).
    pub fn css_scale(&self) -> f32 {
        match self {
            Self::Overview => 0.5,
            Self::Page => 1.0,
            Self::Detail => 1.5,
        }
    }

    /// Next zoom level (cycles Overview → Page → Detail → Overview).
    /// Used by the keyboard shortcut (`Cmd+0`, `Cmd+1`, `Cmd+2`)
    /// or scroll-wheel-while-modifier-held.
    pub fn next(&self) -> Self {
        match self {
            Self::Overview => Self::Page,
            Self::Page => Self::Detail,
            Self::Detail => Self::Overview,
        }
    }

    /// Previous zoom level.
    pub fn prev(&self) -> Self {
        match self {
            Self::Overview => Self::Detail,
            Self::Page => Self::Overview,
            Self::Detail => Self::Page,
        }
    }
}

/// Inspector panel mode. Morphs based on user intent: hidden
/// when the canvas is the focus, minimal during quick edits,
/// full when the user explicitly opens it, floating when
/// detached from the chrome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum InspectorMode {
    /// Inspector not visible. Canvas is full-width.
    #[default]
    Hidden,
    /// Slim inspector showing only the active element's
    /// most-likely-edited properties.
    Minimal,
    /// Full inspector pane with every property the active
    /// element exposes.
    Full,
    /// Inspector detached + draggable. Lives over the canvas
    /// rather than beside it.
    Floating,
}

impl InspectorMode {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Hidden => "hidden",
            Self::Minimal => "minimal",
            Self::Full => "full",
            Self::Floating => "floating",
        }
    }
}

/// Mode discriminator for the ⌘K palette.
///
/// The palette is a single visual surface that switches purpose
/// based on what the user typed. The mode is typed so each
/// consumer (palette renderer, action dispatcher, AI tier) sees
/// exactly which behaviour to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum PaletteMode {
    /// Default — run a known editor command (Save, Toggle dark
    /// mode, Open settings, etc.).
    #[default]
    Commands,
    /// Free-text search over the site's content + pages.
    Search,
    /// Jump-to-page navigation.
    Navigation,
    /// AI prompt — the query is forwarded to the AI tier.
    Ai,
}

impl PaletteMode {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Commands => "commands",
            Self::Search => "search",
            Self::Navigation => "navigation",
            Self::Ai => "ai",
        }
    }
}

/// Runtime state of the ⌘K palette.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct PaletteState {
    /// Whether the palette is currently visible.
    #[serde(default)]
    pub open: bool,
    /// Current query string the user has typed.
    #[serde(default)]
    pub query: String,
    /// Active mode discriminator.
    #[serde(default)]
    pub mode: PaletteMode,
}

impl PaletteState {
    /// Closed state — palette hidden, query cleared, default mode.
    pub fn closed() -> Self {
        Self::default()
    }

    /// Open the palette with the given mode, empty query.
    pub fn open(mode: PaletteMode) -> Self {
        Self {
            open: true,
            query: String::new(),
            mode,
        }
    }
}

/// State of the AI assistance bar.
///
/// Models the full lifecycle so consumers can render a coherent
/// surface for each stage rather than juggling boolean flags.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum AiBarState {
    /// AI bar collapsed / not interacting.
    #[default]
    Off,
    /// AI is showing one-line autocomplete suggestions.
    Suggestions {
        /// Prefix the suggestions are based on.
        prefix: String,
        /// Ordered list of candidate completions.
        candidates: Vec<String>,
    },
    /// AI is actively drafting a longer response.
    Drafting {
        /// Original prompt that started the draft.
        prompt: String,
        /// Tokens generated so far (operator can interrupt at any
        /// point).
        partial: String,
    },
    /// Operator is reviewing a finished AI output before
    /// accepting / rejecting.
    Reviewing {
        /// Original prompt.
        prompt: String,
        /// The full proposed output the AI returned.
        proposal: String,
    },
}

impl AiBarState {
    /// Stable kebab-case slug naming the active variant.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Suggestions { .. } => "suggestions",
            Self::Drafting { .. } => "drafting",
            Self::Reviewing { .. } => "reviewing",
        }
    }

    /// True if the operator's keystrokes should be intercepted
    /// (typing accepts the suggestion / cancels the draft).
    pub fn intercepts_typing(&self) -> bool {
        matches!(self, Self::Suggestions { .. } | Self::Drafting { .. })
    }
}

/// Bundled editor state. Serializable so SSR/CSR transitions
/// don't lose context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct EditorState {
    /// Active canvas zoom.
    #[serde(default)]
    pub zoom: ZoomLevel,
    /// Inspector mode.
    #[serde(default)]
    pub inspector: InspectorMode,
    /// Palette runtime state.
    #[serde(default)]
    pub palette: PaletteState,
    /// AI bar lifecycle state.
    #[serde(default)]
    pub ai_bar: AiBarState,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_level_cycles() {
        assert_eq!(ZoomLevel::Overview.next(), ZoomLevel::Page);
        assert_eq!(ZoomLevel::Page.next(), ZoomLevel::Detail);
        assert_eq!(ZoomLevel::Detail.next(), ZoomLevel::Overview);
        assert_eq!(ZoomLevel::Overview.prev(), ZoomLevel::Detail);
        assert_eq!(ZoomLevel::Page.prev(), ZoomLevel::Overview);
        assert_eq!(ZoomLevel::Detail.prev(), ZoomLevel::Page);
    }

    #[test]
    fn zoom_level_css_scales_are_ordered() {
        assert!(ZoomLevel::Overview.css_scale() < ZoomLevel::Page.css_scale());
        assert!(ZoomLevel::Page.css_scale() < ZoomLevel::Detail.css_scale());
        assert_eq!(ZoomLevel::Page.css_scale(), 1.0);
    }

    #[test]
    fn zoom_level_default_is_page() {
        assert_eq!(ZoomLevel::default(), ZoomLevel::Page);
    }

    #[test]
    fn inspector_mode_slugs_distinct() {
        let modes = [
            InspectorMode::Hidden,
            InspectorMode::Minimal,
            InspectorMode::Full,
            InspectorMode::Floating,
        ];
        let mut seen = std::collections::HashSet::new();
        for m in modes {
            assert!(seen.insert(m.slug()));
        }
    }

    #[test]
    fn palette_open_close_helpers() {
        let p = PaletteState::open(PaletteMode::Search);
        assert!(p.open);
        assert_eq!(p.mode, PaletteMode::Search);
        assert!(p.query.is_empty());

        let c = PaletteState::closed();
        assert!(!c.open);
        assert_eq!(c.mode, PaletteMode::Commands);
    }

    #[test]
    fn ai_bar_off_is_default() {
        assert_eq!(AiBarState::default(), AiBarState::Off);
        assert_eq!(AiBarState::default().slug(), "off");
    }

    #[test]
    fn ai_bar_intercepts_typing_in_active_states() {
        assert!(!AiBarState::Off.intercepts_typing());
        assert!(AiBarState::Suggestions {
            prefix: "hi".into(),
            candidates: vec!["hi there".into()]
        }
        .intercepts_typing());
        assert!(AiBarState::Drafting {
            prompt: "p".into(),
            partial: "out".into()
        }
        .intercepts_typing());
        // Reviewing does NOT intercept — operator decides.
        assert!(!AiBarState::Reviewing {
            prompt: "p".into(),
            proposal: "out".into()
        }
        .intercepts_typing());
    }

    #[test]
    fn editor_state_serde_round_trips() {
        let s = EditorState {
            zoom: ZoomLevel::Detail,
            inspector: InspectorMode::Full,
            palette: PaletteState::open(PaletteMode::Ai),
            ai_bar: AiBarState::Drafting {
                prompt: "write a tagline".into(),
                partial: "Welcome".into(),
            },
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: EditorState = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn editor_state_rejects_unknown_field() {
        let bad = r#"{"zoom":"page","inspector":"hidden","palette":{},"ai-bar":{"state":"off"},"ahem":1}"#;
        let r: Result<EditorState, _> = serde_json::from_str(bad);
        assert!(r.is_err());
    }

    #[test]
    fn ai_bar_serde_internal_tag() {
        let s = AiBarState::Suggestions {
            prefix: "hi".into(),
            candidates: vec!["hi there".into()],
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"state\":\"suggestions\""));
        assert!(json.contains("\"prefix\":\"hi\""));
    }

    #[test]
    fn palette_mode_default_is_commands() {
        assert_eq!(PaletteMode::default(), PaletteMode::Commands);
    }
}
