//! Bearer-token check for the streamable HTTP transport (issue #46).
//!
//! Every request to `/mcp` must carry `Authorization: Bearer <api key>`.
//! The comparison runs over SHA-256 digests with a constant-time fold, so
//! neither the key's length nor the position of the first mismatch leaks
//! through timing.

use axum::{
    extract::{Request, State},
    http::{HeaderValue, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use sha2::{Digest, Sha256};
use std::sync::Arc;

/// SHA-256 digest of the configured API key.
#[derive(Clone)]
pub struct ApiKeyDigest(Arc<[u8; 32]>);

impl ApiKeyDigest {
    /// Hash `key` for later comparison.
    pub fn new(key: &str) -> Self {
        Self(Arc::new(Sha256::digest(key.as_bytes()).into()))
    }

    /// True when `authorization` is `Bearer <key>` for the configured key.
    pub fn matches(&self, authorization: Option<&str>) -> bool {
        let Some(token) = authorization.and_then(|v| v.strip_prefix("Bearer ")) else {
            return false;
        };
        let presented: [u8; 32] = Sha256::digest(token.as_bytes()).into();
        presented
            .iter()
            .zip(self.0.iter())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
    }
}

/// axum middleware: reject requests without the right bearer token with 401.
pub async fn require_bearer(
    State(digest): State<ApiKeyDigest>,
    request: Request,
    next: Next,
) -> Response {
    let authorization = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    if digest.matches(authorization) {
        return next.run(request).await;
    }
    let mut response = (StatusCode::UNAUTHORIZED, "missing or invalid API key").into_response();
    response
        .headers_mut()
        .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_right_key() {
        let d = ApiKeyDigest::new("s3cret-key");
        assert!(d.matches(Some("Bearer s3cret-key")));
    }

    #[test]
    fn rejects_missing_wrong_or_malformed() {
        let d = ApiKeyDigest::new("s3cret-key");
        assert!(!d.matches(None));
        assert!(!d.matches(Some("Bearer wrong")));
        assert!(!d.matches(Some("Bearer s3cret-key ")));
        assert!(!d.matches(Some("Bearer s3cret-ke")));
        assert!(!d.matches(Some("s3cret-key")));
        assert!(!d.matches(Some("Basic s3cret-key")));
        assert!(!d.matches(Some("Bearer ")));
    }
}
