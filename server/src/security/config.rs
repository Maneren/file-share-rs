//! Hardening configuration: parsed CLI limits plus live limiter state.

use std::{
    collections::HashMap,
    net::IpAddr,
    sync::Mutex,
    time::{Duration, Instant},
};

use crate::cli::Config;

/// Max tracked IPs before stale buckets are evicted.
const MAX_TRACKED_IPS: usize = 8192;

/// Idle time after which an IP bucket is evicted during cleanup.
const BUCKET_TTL: Duration = Duration::from_secs(300);

#[derive(Debug)]
pub struct SecurityConfig {
    /// When `Some`, every request needs the token (Bearer or login cookie).
    pub auth_token: Option<String>,
    /// Bounds `/upload` bodies and the browser upload server-fn (`None` =
    /// unlimited).
    pub max_upload_size: Option<u64>,
    /// Aborts an archive stream once its output exceeds this.
    pub max_archive_size: Option<u64>,
    /// Rejects archive trees deeper than this before streaming.
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

    /// Token-bucket check for one IP. Always allows without `--rate-limit`;
    /// fails open if the tracker is over capacity.
    #[must_use]
    pub fn allow_request(&self, ip: IpAddr) -> bool {
        self.rate_limiter
            .as_ref()
            .is_none_or(|limiter| limiter.allow(ip))
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

    /// Returns `true` when the request may proceed.
    fn allow(&self, ip: IpAddr) -> bool {
        let mut buckets = self
            .buckets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if buckets.len() > MAX_TRACKED_IPS {
            buckets.retain(|_, bucket| bucket.last.elapsed() < BUCKET_TTL);
            if buckets.len() > MAX_TRACKED_IPS {
                return true;
            }
        }
        let now = Instant::now();
        let bucket = buckets.entry(ip).or_insert_with(|| Bucket {
            tokens: self.max_burst,
            last: now,
        });
        let elapsed = now.duration_since(bucket.last).as_secs_f64();
        bucket.last = now;
        bucket.tokens = (bucket.tokens + elapsed * self.rate_per_sec).min(self.max_burst);
        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

#[derive(Debug)]
struct Bucket {
    tokens: f64,
    last: Instant,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> Config {
        Config {
            target_dir: std::path::PathBuf::from("."),
            allow_upload: false,
            port: 0,
            qr: false,
            interfaces: Vec::new(),
            auth_token: None,
            rate_limit: None,
            max_upload_size: None,
            max_archive_size: None,
            max_archive_depth: None,
            archive_timeout: None,
        }
    }

    #[test]
    fn burst_then_throttle() {
        let security = SecurityConfig::new(&Config {
            rate_limit: Some(2),
            ..test_config()
        });
        let ip: IpAddr = "127.0.0.1".parse().unwrap();
        assert!(security.allow_request(ip));
        assert!(security.allow_request(ip));
        assert!(!security.allow_request(ip));
    }

    #[test]
    fn disabled_allows_everything() {
        let security = SecurityConfig::new(&test_config());
        let ip: IpAddr = "127.0.0.1".parse().unwrap();
        assert!(security.allow_request(ip));
        assert!(security.allow_request(ip));
    }

    #[test]
    fn token_verification() {
        let security = SecurityConfig::new(&Config {
            auth_token: Some("secret".to_string()),
            ..test_config()
        });
        assert!(security.verify_token("secret"));
        assert!(!security.verify_token("wrong"));
        assert!(!security.verify_token("secre"));
        assert!(!security.verify_token("secret-longer"));
    }
}
