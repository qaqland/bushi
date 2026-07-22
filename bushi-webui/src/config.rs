use std::{env, fs, path::PathBuf, sync::Arc};

use anyhow::{Context, Result, bail};
use axum::http::Uri;
use serde::Deserialize;

const DEFAULT_CONFIG_PATH: &str = "bushi.toml";
const DEFAULT_DATABASE: &str = "test.db";
const DEFAULT_BIND: &str = "127.0.0.1:3000";
const DEFAULT_GITHUB: &str = "https://github.com/qaqland/bushi";

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub database: String,
    pub bind: String,
    pub header_links: Arc<[HeaderLink]>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct HeaderLink {
    pub label: String,
    pub href: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    database: Option<String>,
    bind: Option<String>,
    header_links: Option<Vec<HeaderLink>>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            database: DEFAULT_DATABASE.to_string(),
            bind: DEFAULT_BIND.to_string(),
            header_links: Arc::from(vec![default_github_link()]),
        }
    }
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let file = load_file_config()?;
        Self::from_file(
            file,
            env::var("BUSHI_DATABASE").ok(),
            env::var("BUSHI_BIND").ok(),
        )
    }

    fn from_file(
        file: FileConfig,
        database_override: Option<String>,
        bind_override: Option<String>,
    ) -> Result<Self> {
        let defaults = Self::default();
        let database = database_override
            .or(file.database)
            .unwrap_or(defaults.database);
        let bind = bind_override.or(file.bind).unwrap_or(defaults.bind);
        let links = file
            .header_links
            .unwrap_or_else(|| defaults.header_links.to_vec());
        validate_links(&links)?;

        Ok(Self {
            database,
            bind,
            header_links: Arc::from(links),
        })
    }
}

fn load_file_config() -> Result<FileConfig> {
    let path = env::var_os("BUSHI_CONFIG").map(PathBuf::from).or_else(|| {
        let path = PathBuf::from(DEFAULT_CONFIG_PATH);
        path.is_file().then_some(path)
    });
    let Some(path) = path else {
        return Ok(FileConfig::default());
    };
    let text = fs::read_to_string(&path)
        .with_context(|| format!("read configuration file {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parse configuration file {}", path.display()))
}

fn validate_links(links: &[HeaderLink]) -> Result<()> {
    for link in links {
        if link.label.trim().is_empty() {
            bail!("header link label must not be empty");
        }
        let uri = link
            .href
            .parse::<Uri>()
            .with_context(|| format!("invalid header link URL: {}", link.href))?;
        if !matches!(uri.scheme_str(), Some("http" | "https")) || uri.authority().is_none() {
            bail!(
                "header link URL must be an absolute http:// or https:// URL: {}",
                link.href
            );
        }
    }
    Ok(())
}

fn default_github_link() -> HeaderLink {
    HeaderLink {
        label: "GitHub".to_string(),
        href: DEFAULT_GITHUB.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{AppConfig, FileConfig, HeaderLink, validate_links};

    #[test]
    fn parses_example_config() {
        let file: FileConfig = toml::from_str(include_str!("../bushi.toml.example")).unwrap();
        let config = AppConfig::from_file(file, None, None).unwrap();
        assert_eq!(config.database, "test.db");
        assert_eq!(config.bind, "127.0.0.1:3000");
        assert_eq!(config.header_links.len(), 1);
        assert_eq!(config.header_links[0].label, "GitHub");
        assert_eq!(
            config.header_links[0].href,
            "https://github.com/qaqland/bushi"
        );
    }

    #[test]
    fn file_can_disable_default_links() {
        let file: FileConfig = toml::from_str("header_links = []").unwrap();
        let config = AppConfig::from_file(file, None, None).unwrap();
        assert!(config.header_links.is_empty());
    }

    #[test]
    fn scalar_overrides_win_over_file() {
        let file: FileConfig = toml::from_str(
            r#"
database = "from-file.db"
bind = "127.0.0.1:4000"
"#,
        )
        .unwrap();
        let config = AppConfig::from_file(
            file,
            Some("from-env.db".to_string()),
            Some("127.0.0.1:5000".to_string()),
        )
        .unwrap();
        assert_eq!(config.database, "from-env.db");
        assert_eq!(config.bind, "127.0.0.1:5000");
    }

    #[test]
    fn accepts_http_and_https_links() {
        validate_links(&[
            HeaderLink {
                label: "HTTP".to_string(),
                href: "http://example.com".to_string(),
            },
            HeaderLink {
                label: "HTTPS".to_string(),
                href: "https://example.com".to_string(),
            },
        ])
        .unwrap();
    }

    #[test]
    fn rejects_empty_label() {
        let error = validate_links(&[HeaderLink {
            label: "  ".to_string(),
            href: "https://example.com".to_string(),
        }])
        .unwrap_err();
        assert!(error.to_string().contains("label"));
    }

    #[test]
    fn rejects_non_http_url() {
        let error = validate_links(&[HeaderLink {
            label: "Example".to_string(),
            href: "javascript:alert(1)".to_string(),
        }])
        .unwrap_err();
        assert!(error.to_string().contains("http://"));
    }

    #[test]
    fn rejects_url_without_host() {
        let error = validate_links(&[HeaderLink {
            label: "Example".to_string(),
            href: "https://".to_string(),
        }])
        .unwrap_err();
        assert!(error.to_string().contains("header link URL"));
    }
}
