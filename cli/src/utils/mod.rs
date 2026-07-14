use time::macros::format_description;

pub fn today() -> String {
    let now = time::OffsetDateTime::now_utc();
    now.format(format_description!("[year]-[month]-[day]"))
        .expect("today() format is static and always valid")
}
