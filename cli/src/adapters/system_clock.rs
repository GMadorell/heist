use crate::domain::value::DateValue;
use crate::ports::clock::Clock;
use time::macros::format_description;

pub struct SystemClock;

impl Clock for SystemClock {
    fn today(&self) -> DateValue {
        let now = time::OffsetDateTime::now_utc();
        let s = now
            .format(format_description!("[year]-[month]-[day]"))
            .expect("today() format is static and always valid");
        DateValue::parse("today", &s).expect("now_utc always yields a valid date")
    }
}
