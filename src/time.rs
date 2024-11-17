use chrono::{DateTime, TimeZone, Utc};

pub trait TimeDisplay {
    fn time_ago(&self) -> String;
}

impl<Tz: TimeZone> TimeDisplay for DateTime<Tz> {
    fn time_ago(&self) -> String {
        let now = Utc::now();
        let delta = now.signed_duration_since(self.with_timezone(&Utc));

        if delta.num_seconds() < 60 {
            format!("{}s ago", delta.num_seconds())
        } else if delta.num_minutes() < 60 {
            format!("{}m ago", delta.num_minutes())
        } else if delta.num_hours() < 24 {
            format!("{}h ago", delta.num_hours())
        } else if delta.num_days() < 7 {
            format!("{}d ago", delta.num_days())
        } else {
            let weeks = delta.num_days() / 7;
            format!("{}w ago", weeks)
        }
    }
}

pub fn time_ago(dt: &DateTime<Utc>) -> String {
    dt.time_ago()
}
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_time_ago() {
        let now = Utc::now();

        let seconds_ago = now - Duration::seconds(45);
        let minutes_ago = now - Duration::minutes(30);
        let hours_ago = now - Duration::hours(5);
        let days_ago = now - Duration::days(2);
        let weeks_ago = now - Duration::weeks(3);

        assert_eq!(seconds_ago.time_ago(), "45s ago");
        assert_eq!(minutes_ago.time_ago(), "30m ago");
        assert_eq!(hours_ago.time_ago(), "5h ago");
        assert_eq!(days_ago.time_ago(), "2d ago");
        assert_eq!(weeks_ago.time_ago(), "3w ago");
    }
}
