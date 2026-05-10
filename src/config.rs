use std::env;
use std::path::PathBuf;
use time::Duration;
use toml_edit::DocumentMut;

pub struct Config {
    pub min_age: Duration,
    pub port: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            min_age: Duration::days(14),
            port: 2015,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let path = cargo_config_path();
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let doc = text.parse::<DocumentMut>().unwrap_or_default();

        let mut cfg = Self::default();

        if let Some(item) = doc
            .get("registry")
            .and_then(|t| t.get("global-min-publish-age"))
        {
            if let Some(s) = item.as_str() {
                if let Some(d) = parse_age(s) {
                    cfg.min_age = d;
                }
            }
        }

        if let Some(item) = doc.get("age-filter").and_then(|t| t.get("port")) {
            if let Some(n) = item.as_integer() {
                if n > 0 && n <= u16::MAX as i64 {
                    cfg.port = n as u16;
                }
            }
        }

        cfg
    }
}

fn cargo_config_path() -> PathBuf {
    let cargo_home = env::var_os("CARGO_HOME").map(PathBuf::from).or_else(|| {
        #[cfg(windows)]
        let home = env::var_os("USERPROFILE")?;
        #[cfg(not(windows))]
        let home = env::var_os("HOME")?;
        Some(PathBuf::from(home).join(".cargo"))
    });
    cargo_home
        .expect("CARGO_HOME or HOME/USERPROFILE must be set")
        .join("config.toml")
}

fn parse_age(s: &str) -> Option<Duration> {
    let s = s.trim();
    if s == "0" {
        return Some(Duration::ZERO);
    }
    let mut parts = s.split_whitespace();
    let n: i64 = parts.next()?.parse().ok()?;
    let unit = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let unit = unit.trim_end_matches('s');
    match unit {
        "second" => Some(Duration::seconds(n)),
        "minute" => Some(Duration::minutes(n)),
        "hour" => Some(Duration::hours(n)),
        "day" => Some(Duration::days(n)),
        "week" => Some(Duration::weeks(n)),
        "month" => Some(Duration::days(n * 30)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_age;
    use time::Duration;

    #[test]
    fn test_parse_age() {
        assert_eq!(parse_age("0"), Some(Duration::ZERO));
        assert_eq!(parse_age("14 days"), Some(Duration::days(14)));
        assert_eq!(parse_age("2 weeks"), Some(Duration::days(14)));
        assert_eq!(parse_age("1 month"), Some(Duration::days(30)));
        assert_eq!(parse_age("48 hours"), Some(Duration::hours(48)));
        assert_eq!(parse_age("12 hours"), Some(Duration::hours(12)));
        assert_eq!(parse_age("not a duration"), None);
        assert_eq!(parse_age("14 unknown"), None);
    }
}
