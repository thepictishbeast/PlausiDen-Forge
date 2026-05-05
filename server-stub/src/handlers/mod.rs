//! Handler module list. Each entry corresponds to one
//! [backends.X] section in backends.toml (key dashes →
//! underscores). Add a `pub mod <name>;` line here when
//! loom backend-stub scaffolds a new handler file.

pub mod cash_out;
pub mod cast_vote;
pub mod challenge_create;
pub mod enter_challenge;
pub mod follow;
pub mod list_challenges;
pub mod list_leaderboard;
pub mod list_open_votes;
pub mod list_touches;
pub mod live_start;
pub mod post_photo;
pub mod post_skill;
pub mod report_challenge;
pub mod report_profile;
pub mod sign_in;
pub mod sign_out;
pub mod sign_up;
pub mod upload_entry;
pub mod view_challenge;
pub mod view_profile;
