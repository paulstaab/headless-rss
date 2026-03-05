use std::env;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct Config {
    pub username: Option<String>,
    pub password: Option<String>,
    pub version: String,
    pub db_path: String,
}

impl Config {
    pub fn from_env() -> Self {
        let db_path = env::var("DATABASE_PATH").unwrap_or_else(|_| default_db_path());

        Self {
            username: get_env_str("USERNAME"),
            password: get_env_str("PASSWORD"),
            version: env::var("VERSION").unwrap_or_else(|_| "dev".to_string()),
            db_path,
        }
    }

    pub fn auth_enabled(&self) -> bool {
        self.username.is_some() && self.password.is_some()
    }
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
