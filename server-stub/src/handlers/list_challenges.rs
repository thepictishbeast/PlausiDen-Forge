//! `list-challenges` — backend handler stub.
//!
//! Method: GET
//! Path:   /challenges
//! Purpose: battle feed list
//!
//! Scaffolded by `loom backend-stub`. Replace the placeholder
//! Request/Response types and the handler body with the real
//! implementation. Update the test below to exercise the
//! actual semantics.

use serde::{Deserialize, Serialize};

/// `list-challenges` request payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    // TODO: declare request fields.
}

/// `list-challenges` response payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    /// Always `true` on success; absent on error.
    pub ok: bool,
}

/// Handler entry point. Wire into your axum/actix/rocket
/// router at `GET /challenges`.
///
/// AVP-2: returns `Result<Response, anyhow::Error>` so caller
/// chooses how to translate the error to an HTTP response
/// (typically 4xx for client error, 5xx for server error).
pub async fn handle_get(_req: Request) -> Result<Response, anyhow::Error> {
    // TODO: implement list-challenges (battle feed list).
    Ok(Response { ok: true })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn placeholder_returns_ok() {
        let resp = handle_get(Request {}).await.expect("ok");
        assert!(resp.ok);
    }

    #[test]
    fn module_name_matches_key() {
        // Self-doc: this file lives at src/handlers/list_challenges.rs
        // for backend key "list-challenges".
        assert_eq!("list_challenges", "list_challenges");
    }
}
