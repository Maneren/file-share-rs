use std::time::{self, UNIX_EPOCH};

use chrono::{DateTime, TimeZone, Utc};
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
      }
    };
    Self(sec, nsec)
  }
}
impl From<SystemTime> for DateTime<Utc> {
  #[allow(clippy::similar_names)]
  fn from(time: SystemTime) -> Self {
    let SystemTime(sec, nsec) = time;
    Utc.timestamp_opt(sec, nsec).unwrap()
  }
}
// impl From<SystemTime> for HumanTime {
//   fn from(time: SystemTime) -> Self {
//     HumanTime::from(DateTime::<Utc>::from(time))
//   }
// }
// impl Humanize for SystemTime {
//   fn humanize(&self) -> String {
//     HumanTime::from(*self).to_string()
//   }
// }
// impl From<SystemTime> for DateTime<Local> {
//   fn from(time: SystemTime) -> DateTime<Local> {
//     DateTime::<Utc>::from(time).with_timezone(&Local)
//   }
// }
