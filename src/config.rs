//! Global config (`~/.flute/config.toml`) and environment profiles.

use std::path::PathBuf;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Config {
    pub default_profile: String,
    pub output: String,
    pub auto_update_check: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_profile: "sandbox".into(),
            output: "table".into(),
            auto_update_check: true,
        }
    }
}

pub fn config_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".flute")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn load_or_default() -> Config {
    match std::fs::read_to_string(config_path()) {
        Ok(text) => toml::from_str(&text).unwrap_or_default(),
        Err(_) => Config::default(),
    }
}

/// Persist the global config (used by `auth switch` to set the default profile).
pub fn save(cfg: &Config) -> anyhow::Result<()> {
    std::fs::create_dir_all(config_dir())?;
    std::fs::write(config_path(), toml::to_string_pretty(cfg)?)?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub name: String,
    pub api_base_url: String,
    pub oauth_url: String,
}

impl Profile {
    pub fn sandbox() -> Self {
        // NOTE: `sandbox` points at UAT until the real sandbox env ships.
        Self {
            name: "sandbox".into(),
            api_base_url: "https://api.uat.arise.risewithaurora.com".into(),
            oauth_url: "https://oauth.api.uat.arise.risewithaurora.com/oauth2/token".into(),
        }
    }

    pub fn production() -> Self {
        Self {
            name: "production".into(),
            api_base_url: "https://api.arise.risewithaurora.com".into(),
            oauth_url: "https://oauth.arise.risewithaurora.com/oauth2/token".into(),
        }
    }

    pub fn by_name(name: &str) -> Option<Self> {
        match name {
            "sandbox" => Some(Self::sandbox()),
            "production" | "prod" => Some(Self::production()),
            _ => None,
        }
    }

    pub fn is_production(&self) -> bool {
        self.name == "production"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_points_at_uat() {
        let p = Profile::sandbox();
        assert_eq!(p.name, "sandbox");
        assert_eq!(p.api_base_url, "https://api.uat.arise.risewithaurora.com");
        assert_eq!(
            p.oauth_url,
            "https://oauth.api.uat.arise.risewithaurora.com/oauth2/token"
        );
    }

    #[test]
    fn production_points_at_prod() {
        let p = Profile::production();
        assert_eq!(p.api_base_url, "https://api.arise.risewithaurora.com");
        assert_eq!(
            p.oauth_url,
            "https://oauth.arise.risewithaurora.com/oauth2/token"
        );
    }

    #[test]
    fn by_name_resolves_aliases_and_rejects_unknown() {
        assert_eq!(Profile::by_name("sandbox").unwrap().name, "sandbox");
        assert_eq!(Profile::by_name("production").unwrap().name, "production");
        assert_eq!(Profile::by_name("prod").unwrap().name, "production");
        assert!(Profile::by_name("garbage").is_none());
    }

    #[test]
    fn base_url_has_no_path_prefix() {
        for p in [Profile::sandbox(), Profile::production()] {
            let after_host = p.api_base_url.trim_start_matches("https://");
            assert!(!after_host.contains('/'), "{} must have no path", p.name);
        }
    }

    #[test]
    fn config_default_is_sandbox() {
        assert_eq!(Config::default().default_profile, "sandbox");
    }
}
