//! Server-only hardening: auth token, per-IP rate limiting, size/timeout caps.
//!
//! Everything here is opt-in via CLI flags (`--auth-token`, `--rate-limit`,
//! `--max-upload-size`, `--max-archive-size`, `--max-archive-depth`,
//! `--archive-timeout`) and, unlike [`AppConfig`](file_share_app::AppConfig),
//! never leaves the server: the token in particular must not reach the client.

use std::{
    collections::HashMap,
    net::IpAddr,
    sync::Mutex,
    time::{Duration, Instant},
};

use axum::{
    Form,
    body::Body,
    extract::State,
    http::{HeaderValue, Request, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use serde::Deserialize;

use crate::{cli::Config, state::AppState};

/// Name of the login cookie carrying the shared token.
const AUTH_COOKIE: &str = "fs_auth";

/// How long the login cookie lasts (30 days).
const COOKIE_MAX_AGE: u64 = 30 * 24 * 60 * 60;

/// Cap on tracked IPs before stale buckets are evicted (memory DoS guard).
const MAX_TRACKED_IPS: usize = 8192;

/// Idle time after which an IP bucket is evicted during cleanup.
const BUCKET_TTL: Duration = Duration::from_secs(300);

#[derive(Debug)]
pub struct SecurityConfig {
    /// When `Some`, every request needs `Authorization: Bearer <token>` or
    /// the login cookie (see [`auth`](self::auth) middleware).
    pub auth_token: Option<String>,
    /// Bounds `/upload` bodies and the browser upload server-fn.
    /// `None` keeps the historical unlimited behavior.
    pub max_upload_size: Option<u64>,
    /// Aborts an archive stream once its total output exceeds this.
    pub max_archive_size: Option<u64>,
    /// Rejects archive trees deeper than this before streaming starts.
    pub max_archive_depth: Option<usize>,
    /// Aborts archive generation after this long.
    pub archive_timeout: Option<Duration>,
    rate_limiter: Option<RateLimiter>,
}

impl SecurityConfig {
    #[must_use]
    pub fn new(config: &Config) -> Self {
        Self {
            auth_token: config.auth_token.clone(),
            max_upload_size: config.max_upload_size,
            max_archive_size: config.max_archive_size,
            max_archive_depth: config.max_archive_depth,
            archive_timeout: config
                .archive_timeout
                .map(Duration::from_secs)
                .filter(|d| !d.is_zero()),
            rate_limiter: config.rate_limit.map(RateLimiter::new),
        }
    }

    /// Whether any request must carry the shared token.
    #[must_use]
    pub fn auth_enabled(&self) -> bool {
        self.auth_token.is_some()
    }

    /// Check `token` against the configured secret in constant time.
    #[must_use]
    pub fn verify_token(&self, token: &str) -> bool {
        self.auth_token.as_deref().is_some_and(|expected| {
            let (a, b) = (expected.as_bytes(), token.as_bytes());
            a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .fold(0u8, |diff, (x, y)| diff | (x ^ y))
                    == 0
        })
    }

    /// Token-bucket check for one IP. Returns `true` when the request may
    /// proceed. Always allows when no `--rate-limit` was passed; fails open
    /// (allows) if the tracker is over capacity.
    #[must_use]
    pub fn allow_request(&self, ip: IpAddr) -> bool {
        self.rate_limiter.as_ref().is_none_or(|limiter| {
            let mut buckets = limiter.buckets.lock().unwrap_or_else(|e| e.into_inner());
            if buckets.len() > MAX_TRACKED_IPS {
                buckets.retain(|_, bucket| bucket.last.elapsed() < BUCKET_TTL);
                if buckets.len() > MAX_TRACKED_IPS {
                    return true;
                }
            }
            let now = Instant::now();
            let bucket = buckets.entry(ip).or_insert_with(|| Bucket {
                tokens: limiter.max_burst,
                last: now,
            });
            let elapsed = now.duration_since(bucket.last).as_secs_f64();
            bucket.last = now;
            bucket.tokens = (bucket.tokens + elapsed * limiter.rate_per_sec).min(limiter.max_burst);
            if bucket.tokens >= 1.0 {
                bucket.tokens -= 1.0;
                true
            } else {
                false
            }
        })
    }
}

#[derive(Debug)]
struct RateLimiter {
    rate_per_sec: f64,
    max_burst: f64,
    buckets: Mutex<HashMap<IpAddr, Bucket>>,
}

impl RateLimiter {
    fn new(requests_per_sec: u32) -> Self {
        let rate = f64::from(requests_per_sec);
        Self {
            rate_per_sec: rate,
            max_burst: rate,
            buckets: Mutex::default(),
        }
    }
}

#[derive(Debug)]
struct Bucket {
    tokens: f64,
    last: Instant,
}

/// Require the shared `--auth-token` on every request when configured.
///
/// Accepts `Authorization: Bearer <token>` (curl/API) or the `fs_auth`
/// cookie set by `POST /login` (browser UI fetches carry cookies, so the web
/// UI keeps working after one login). No-op when no token is configured.
pub async fn require_auth(
    State(app_state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let security = &app_state.security;
    if !security.auth_enabled() || req.uri().path() == "/login" {
        return next.run(req).await;
    }

    let authorized = bearer_token(&req)
        .map(str::to_owned)
        .or_else(|| cookie_token(&req))
        .is_some_and(|token| security.verify_token(&token));
    if authorized {
        return next.run(req).await;
    }

    let mut response = if wants_html(&req) {
        login_page(safe_next(req.uri().path())).into_response()
    } else {
        (StatusCode::UNAUTHORIZED, "Unauthorized").into_response()
    };
    response.headers_mut().insert(
        header::WWW_AUTHENTICATE,
        HeaderValue::from_static("Bearer"),
    );
    *response.status_mut() = StatusCode::UNAUTHORIZED;
    response
}

#[derive(Deserialize)]
pub struct LoginForm {
    token: String,
    next: Option<String>,
}

/// Verify the token from the login form; on success set the cookie and
/// redirect back, otherwise `403` without saying why.
pub async fn login(
    State(app_state): State<AppState>,
    Form(form): Form<LoginForm>,
) -> Response {
    if !app_state.security.auth_enabled() || !app_state.security.verify_token(&form.token) {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let target = form.next.map_or_else(|| "/".to_string(), |n| safe_next(&n));
    let cookie = format!(
        "{AUTH_COOKIE}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={COOKIE_MAX_AGE}",
        urlencoding::encode(&form.token),
    );
    ([(header::SET_COOKIE, cookie)], Redirect::to(&target)).into_response()
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
    cookies.split(';').find_map(|pair| {
        let (name, value) = pair.split_once('=')?;
        (name.trim() == AUTH_COOKIE)
            .then(|| urlencoding::decode(value.trim()).ok().map(|v| v.into_owned()))
            .flatten()
    })
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
    if valid { next.to_string() } else { "/".to_string() }
}

/// `401` page with an inline login form (avoids a redirect round-trip).
fn login_page(next: String) -> (StatusCode, [(header::HeaderName, &'static str); 1], String) {
    let body = format!(
        "<!DOCTYPE html><html><body><h1>Login required</h1>\
        <form method=\"post\" action=\"/login\">\
        <input type=\"hidden\" name=\"next\" value=\"{next}\">\
        <input type=\"password\" name=\"token\" placeholder=\"Access token\" autofocus>\
        <button type=\"submit\">Log in</button>\
        </form></body></html>"
    );
    (
        StatusCode::UNAUTHORIZED,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        body,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with_rate_limit(rps: u32) -> Config {
        Config {
            target_dir: std::path::PathBuf::from("."),
            allow_upload: false,
            port: 0,
            qr: false,
            interfaces: Vec::new(),
            auth_token: None,
            rate_limit: Some(rps),
            max_upload_size: None,
            max_archive_size: None,
            max_archive_depth: None,
            archive_timeout: None,
        }
    }

    #[test]
    fn burst_then_throttle() {
        let security = SecurityConfig::new(&config_with_rate_limit(2));
        let ip: IpAddr = "127.0.0.1".parse().unwrap();
        assert!(security.allow_request(ip));
        assert!(security.allow_request(ip));
        assert!(!security.allow_request(ip));
    }

    #[test]
    fn disabled_allows_everything() {
        let mut config = config_with_rate_limit(1);
        config.rate_limit = None;
        let security = SecurityConfig::new(&config);
        let ip: IpAddr = "127.0.0.1".parse().unwrap();
        assert!(security.allow_request(ip));
        assert!(security.allow_request(ip));
    }

    #[test]
    fn token_verification() {
        let mut config = config_with_rate_limit(1);
        config.rate_limit = None;
        config.auth_token = Some("secret".to_string());
        let security = SecurityConfig::new(&config);
        assert!(security.verify_token("secret"));
        assert!(!security.verify_token("wrong"));
        assert!(!security.verify_token("secre"));
        assert!(!security.verify_token("secret-longer"));
    }

    #[test]
    fn redirect_targets_stay_same_origin() {
        assert_eq!(safe_next("/files/a%20b"), "/files/a%20b");
        assert_eq!(safe_next("/"), "/");
        assert_eq!(safe_next("https://evil.example"), "/");
        assert_eq!(safe_next("//evil.example"), "/");
        assert_eq!(safe_next("/x\" onload=\"y"), "/");
        assert_eq!(safe_next(""), "/");
    }
}
