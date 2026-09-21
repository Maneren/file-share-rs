use std::time::{self, UNIX_EPOCH};

use chrono::{DateTime, TimeZone, Utc};
use chrono_humanize::Humanize;
use leptos::prelude::IntoRender;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct SystemTime(pub i64, pub u32);

impl From<time::SystemTime> for SystemTime {
    #[allow(clippy::similar_names)]
    #[allow(clippy::cast_possible_wrap)]
    fn from(time: time::SystemTime) -> Self {
        let (sec, nsec) = match time.duration_since(UNIX_EPOCH) {
            Ok(dur) => (dur.as_secs() as i64, dur.subsec_nanos()),
            Err(e) => {
                // unlikely but should be handled
                let dur = e.duration();
                let (sec, nsec) = (dur.as_secs() as i64, dur.subsec_nanos());
                if nsec == 0 {
                    (-sec, 0)
                } else {
                    (-sec - 1, 1_000_000_000 - nsec)
                }
            },
        };
        Self(sec, nsec)
    }
}

impl From<SystemTime> for DateTime<Utc> {
    #[allow(clippy::similar_names)]
    fn from(time: SystemTime) -> Self {
        let SystemTime(sec, nsec) = time;
        // Out-of-range timestamps (settable via `touch`) yield `None`;
        // fall back to the epoch instead of panicking the listing.
        Utc.timestamp_opt(sec, nsec)
            .single()
            .unwrap_or(DateTime::<Utc>::UNIX_EPOCH)
    }
}

impl IntoRender for SystemTime {
    type Output = String;

    fn into_render(self) -> Self::Output {
        DateTime::from(self)
            .format("%Y-%m-%d %H:%M:%S UTC")
            .to_string()
    }
}

impl SystemTime {
    #[must_use]
    pub fn humanize(&self) -> String {
        DateTime::from(*self).humanize()
    }
}
