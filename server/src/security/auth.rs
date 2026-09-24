//! Auth and rate-limit middleware plus the `/login` handler.

use std::borrow::Cow;

use axum::{
    Form,
    body::Body,
    extract::{ConnectInfo, State},
    http::{HeaderValue, Request, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use serde::Deserialize;

use crate::state::AppState;

/// Login cookie name.
const AUTH_COOKIE: &str = "fs_auth";

/// Login cookie lifetime: 30 days.
const COOKIE_MAX_AGE: u64 = 30 * 24 * 60 * 60;

/// Require the shared `--auth-token` on every request when configured.
///
/// Accepts `Authorization: Bearer <token>` or the `fs_auth` cookie from
/// `POST /login`. Browsers asking for HTML are redirected to the login
/// page; API clients get a bare `401`. No-op without the flag.
pub async fn require_auth(
    State(app_state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let security = &app_state.security;
    if !security.auth_enabled() || is_public_path(req.uri().path()) {
        return next.run(req).await;
    }

    let authorized = bearer_token(&req)
        .map(str::to_owned)
        .or_else(|| cookie_token(&req))
        .is_some_and(|token| security.verify_token(&token));
    if authorized {
        return next.run(req).await;
    }

    if wants_html(&req) {
        let original = req
            .uri()
            .path_and_query()
            .map_or("/", |original| original.as_str());
        let target = format!("/login?next={}", urlencoding::encode(original));
        return Redirect::to(&target).into_response();
    }

    let mut response = (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    response
        .headers_mut()
        .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
    response
}

/// Per-IP token-bucket rate limiting (`--rate-limit`).
///
/// Runs outside auth so floods are shed first. No-op without the flag; fails
/// open when the peer address is unknown.
pub async fn rate_limit(
    State(app_state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let allowed = req
        .extensions()
        .get::<ConnectInfo<std::net::SocketAddr>>()
        .is_none_or(|ConnectInfo(addr)| app_state.security.allow_request(addr.ip()));
    if allowed {
        return next.run(req).await;
    }
    (
        StatusCode::TOO_MANY_REQUESTS,
        [(header::RETRY_AFTER, "1")],
        "Too many requests",
    )
        .into_response()
}

#[derive(Deserialize)]
pub struct LoginForm {
    token: String,
    next: Option<String>,
}

/// Verify the login-form token; on success set the cookie and redirect back,
/// otherwise bounce to the login page with an error. Without `--auth-token`
/// there is nothing to log into, so go home.
pub async fn login(State(app_state): State<AppState>, Form(form): Form<LoginForm>) -> Response {
    let next = form.next.map_or_else(|| "/".to_string(), |n| safe_next(&n));
    if !app_state.security.auth_enabled() {
        return Redirect::to("/").into_response();
    }
    if !app_state.security.verify_token(&form.token) {
        let target = format!("/login?next={}&error=1", urlencoding::encode(&next));
        return Redirect::to(&target).into_response();
    }
    let cookie = format!(
        "{AUTH_COOKIE}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={COOKIE_MAX_AGE}",
        urlencoding::encode(&form.token),
    );
    ([(header::SET_COOKIE, cookie)], Redirect::to(&next)).into_response()
}

/// Paths that stay public when auth is on: the login flow itself plus the
/// build assets the login page needs. They carry no secrets.
fn is_public_path(path: &str) -> bool {
    path == "/login" || path == "/favicon.ico" || path.starts_with("/pkg/")
}

/// Extract `Authorization: Bearer <token>`, if present and well-formed.
fn bearer_token(req: &Request<Body>) -> Option<&str> {
    req.headers()
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|token| !token.is_empty())
}

/// Extract the login cookie value, if present and well-formed.
fn cookie_token(req: &Request<Body>) -> Option<String> {
    let cookies = req.headers().get(header::COOKIE)?.to_str().ok()?;
    cookies
        .split(';')
        .filter_map(|pair| pair.split_once('='))
        .find(|(name, _)| name.trim() == AUTH_COOKIE)
        .and_then(|(_, value)| urlencoding::decode(value.trim()).ok())
        .map(Cow::into_owned)
}

fn wants_html(req: &Request<Body>) -> bool {
    req.headers()
        .get(header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|accept| accept.contains("text/html"))
}

/// Keep only same-origin redirect targets; everything else becomes `/`.
fn safe_next(next: &str) -> String {
    let valid = next.starts_with('/')
        && !next.starts_with("//")
        && next
            .chars()
            .all(|c| !c.is_control() && !matches!(c, '"' | '\'' | '<' | '>' | '\\' | ' '));
    if valid {
        next.to_string()
    } else {
        "/".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redirect_targets_stay_same_origin() {
        assert_eq!(safe_next("/files/a%20b"), "/files/a%20b");
        assert_eq!(safe_next("/"), "/");
        assert_eq!(safe_next("https://evil.example"), "/");
        assert_eq!(safe_next("//evil.example"), "/");
        assert_eq!(safe_next("/x\" onload=\"y"), "/");
        assert_eq!(safe_next(""), "/");
    }

    #[test]
    fn login_assets_stay_public() {
        assert!(is_public_path("/login"));
        assert!(is_public_path("/pkg/file-share.css"));
        assert!(is_public_path("/pkg/file-share.js"));
        assert!(is_public_path("/favicon.ico"));
        assert!(!is_public_path("/"));
        assert!(!is_public_path("/index"));
        assert!(!is_public_path("/files/a.txt"));
        assert!(!is_public_path("/pkg-notes"));
    }
}
