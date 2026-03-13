use std::env;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct Config {
    pub username: Option<String>,
    pub password: Option<String>,
    pub version: String,
    pub db_path: String,
    pub feed_update_frequency_min: i64,
    pub openai_api_key: Option<String>,
    pub openai_base_url: String,
    pub openai_model: String,
    pub openai_timeout_seconds: u64,
    pub testing_mode: bool,
}

impl Config {
    pub fn from_env() -> Self {
        let db_path = env::var("DATABASE_PATH").unwrap_or_else(|_| default_db_path());

        Self {
            username: get_env_str("USERNAME"),
            password: get_env_str("PASSWORD"),
            version: env::var("VERSION").unwrap_or_else(|_| "dev".to_string()),
            db_path,
            feed_update_frequency_min: get_env_int("FEED_UPDATE_FREQUENCY_MIN", 15),
            openai_api_key: get_env_str("OPENAI_API_KEY"),
            openai_base_url: env::var("OPENAI_BASE_URL")
                .unwrap_or_else(|_| "https://api.openai.com/v1".to_string()),
            openai_model: env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-5-nano".to_string()),
            openai_timeout_seconds: get_env_u64("OPENAI_TIMEOUT_SECONDS", 30),
            testing_mode: env::var("TESTING_MODE")
                .ok()
                .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
                .unwrap_or(cfg!(test)),
        }
    }

    pub fn auth_enabled(&self) -> bool {
        self.username.is_some() && self.password.is_some()
    }

    pub fn llm_enabled(&self) -> bool {
        self.openai_api_key.is_some()
    }
}

fn get_env_int(name: &str, default: i64) -> i64 {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn get_env_u64(name: &str, default: u64) -> u64 {
    env::var(name)
        .ok()
        .as_deref()
        .and_then(parse_positive_u64)
        .unwrap_or(default)
}

fn parse_positive_u64(value: &str) -> Option<u64> {
    value.trim().parse::<u64>().ok().filter(|value| *value > 0)
}

fn get_env_str(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn default_db_path() -> String {
    if Path::new("data/headless-rss.sqlite3").exists() {
        return "data/headless-rss.sqlite3".to_string();
    }

    if Path::new("../data/headless-rss.sqlite3").exists() {
        return "../data/headless-rss.sqlite3".to_string();
    }

    "data/headless-rss.sqlite3".to_string()
}

#[cfg(test)]
mod tests {
    use super::parse_positive_u64;

    #[test]
    fn parse_positive_u64_accepts_positive_integer() {
        assert_eq!(parse_positive_u64("30"), Some(30));
    }

    #[test]
    fn parse_positive_u64_rejects_zero_and_invalid_values() {
        assert_eq!(parse_positive_u64("0"), None);
        assert_eq!(parse_positive_u64("-5"), None);
        assert_eq!(parse_positive_u64("abc"), None);
    }
}
