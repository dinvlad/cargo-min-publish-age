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

        if let Some(item) = doc
            .get("source")
            .and_then(|s| s.get("min-publish-age"))
            .and_then(|m| m.get("registry"))
        {
            if let Some(url) = item.as_str() {
                if let Some(p) = parse_registry_port(url) {
                    cfg.port = p;
                }
            }
        }

        cfg
    }
}

pub fn write_install_config(port: u16, age: &str) -> Result<(), Box<dyn std::error::Error>> {
    let path = cargo_config_path();
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc = text.parse::<DocumentMut>().unwrap_or_default();

    doc["registry"] = toml_edit::table();
    doc["registry"]["global-min-publish-age"] = toml_edit::value(age);

    doc["source"] = toml_edit::table();
    doc["source"].as_table_mut().unwrap().set_implicit(true);

    doc["source"]["crates-io"] = toml_edit::table();
    doc["source"]["crates-io"]["replace-with"] = toml_edit::value("min-publish-age");

    let registry_url = format!("sparse+http://127.0.0.1:{}/index/", port);
    doc["source"]["min-publish-age"] = toml_edit::table();
    doc["source"]["min-publish-age"]["registry"] = toml_edit::value(registry_url);

    std::fs::write(&path, doc.to_string())?;
    Ok(())
}

pub fn remove_install_config() -> Result<(), Box<dyn std::error::Error>> {
    let path = cargo_config_path();
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc = text.parse::<DocumentMut>().unwrap_or_default();

    if let Some(source) = doc.get_mut("source").and_then(|s| s.as_table_mut()) {
        source.remove("min-publish-age");

        let should_remove_crates_io = source
            .get("crates-io")
            .and_then(|item| item.as_table())
            .and_then(|tbl| tbl.get("replace-with"))
            .and_then(|item| item.as_str())
            == Some("min-publish-age");

        if should_remove_crates_io {
            source.remove("crates-io");
        }

        if source.is_empty() {
            doc.as_table_mut().remove("source");
        }
    }

    if let Some(registry) = doc.get_mut("registry").and_then(|r| r.as_table_mut()) {
        registry.remove("global-min-publish-age");
        if registry.is_empty() {
            doc.as_table_mut().remove("registry");
        }
    }

    std::fs::write(&path, doc.to_string())?;
    Ok(())
}

pub fn cargo_config_path() -> PathBuf {
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

fn parse_registry_port(url: &str) -> Option<u16> {
    let before_path = url.strip_suffix("/index/")?;
    let colon_pos = before_path.rfind(':')?;
    let port_str = &before_path[colon_pos + 1..];
    port_str.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::{
        parse_age, parse_registry_port, remove_install_config, write_install_config, Config,
    };
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

    #[test]
    fn test_parse_registry_port() {
        assert_eq!(
            parse_registry_port("sparse+http://127.0.0.1:2015/index/"),
            Some(2015)
        );
        assert_eq!(
            parse_registry_port("sparse+https://127.0.0.1:8080/index/"),
            Some(8080)
        );
        assert_eq!(parse_registry_port("not a url"), None);
    }

    #[test]
    fn test_write_and_remove_install_config() {
        let tmp_dir = std::env::temp_dir().join("cargo-min-publish-age-test");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let cfg_path = tmp_dir.join("config.toml");
        std::env::set_var("CARGO_HOME", &tmp_dir);

        write_install_config(3000, "7 days").unwrap();

        let text = std::fs::read_to_string(&cfg_path).unwrap();
        assert!(text.contains("global-min-publish-age = \"7 days\""));
        assert!(text.contains("replace-with = \"min-publish-age\""));
        assert!(text.contains("registry = \"sparse+http://127.0.0.1:3000/index/\""));

        let cfg = Config::load();
        assert_eq!(cfg.port, 3000);
        assert_eq!(cfg.min_age, Duration::days(7));

        remove_install_config().unwrap();
        let text = std::fs::read_to_string(&cfg_path).unwrap();
        assert!(!text.contains("[source.min-publish-age]"));
        assert!(!text.contains("[source.crates-io]"));
        assert!(!text.contains("[registry]"));

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
}
