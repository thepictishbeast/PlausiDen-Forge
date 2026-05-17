//! Which subsystem owns a capability.
//!
//! Each capability belongs to exactly one subsystem. The
//! `manifest-codegen` crate uses this to decide which downstream
//! crate's projection a capability lands in.

use serde::{Deserialize, Serialize};

/// Subsystems the platform is composed of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Ownership {
    /// PlausiDen-Forge — build pipeline + audit.
    #[default]
    Forge,
    /// PlausiDen-Loom — typed design primitives + tokens.
    Loom,
    /// PlausiDen-Crawler — runtime audit, journey recording.
    Crawler,
    /// PlausiDen-Annotator — desktop UX capture.
    Annotator,
    /// PlausiDen-CMS — typed CMS pages + admin editor.
    Cms,
}

impl Ownership {
    /// All variants, in declaration order. Used by the coverage
    /// gate (task #33) to enumerate per-subsystem statistics.
    pub const ALL: &'static [Ownership] = &[
        Ownership::Forge,
        Ownership::Loom,
        Ownership::Crawler,
        Ownership::Annotator,
        Ownership::Cms,
    ];

    /// Stable kebab-case slug for serialization + UI.
    pub fn slug(&self) -> &'static str {
        match self {
            Ownership::Forge => "forge",
            Ownership::Loom => "loom",
            Ownership::Crawler => "crawler",
            Ownership::Annotator => "annotator",
            Ownership::Cms => "cms",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_variants_have_distinct_slugs() {
        let mut seen = std::collections::HashSet::new();
        for o in Ownership::ALL {
            assert!(seen.insert(o.slug()), "duplicate slug for {o:?}");
        }
        assert_eq!(seen.len(), 5);
    }

    #[test]
    fn slug_round_trips_through_serde() {
        for o in Ownership::ALL {
            let s = serde_json::to_string(o).unwrap();
            let back: Ownership = serde_json::from_str(&s).unwrap();
            assert_eq!(*o, back);
        }
    }
}
