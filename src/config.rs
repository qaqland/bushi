use std::{collections::HashMap, path::PathBuf};

#[derive(Debug, serde::Deserialize)]
pub struct Config {
    pub web: Web,
    pub path: PathBuf,
    pub repo: Vec<Repo>,
}

#[derive(Debug, serde::Deserialize)]
pub struct Web {
    pub name: String,
    pub desc: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct Repo {
    #[serde(default = "default_zero")]
    pub repo_id: i64,
    pub name: String,
    pub desc: String,
    pub head: String,
    pub path: PathBuf,
}

const fn default_zero() -> i64 {
    0
}

impl Config {
    /// Get config from `BUSHI_CONFIG` or `bushi.toml` in current directory
    pub fn new() -> Result<Self, config::ConfigError> {
        let name = std::env::var("BUSHI_CONFIG").unwrap_or("bushi".to_string());
        let c = config::Config::builder()
            .add_source(config::File::with_name(&name).format(config::FileFormat::Toml))
            .build()?;
        c.try_deserialize::<Self>()
    }

    pub fn canonicalize(&mut self) -> Result<(), std::io::Error> {
        self.path = std::fs::canonicalize(&self.path)?;
        for repo in &mut self.repo {
            repo.path = std::fs::canonicalize(&repo.path)?;
        }
        Ok(())
    }

    pub fn init_marks(&self) -> Result<u32, std::io::Error> {
        let mut count = 0;
        for repo in &self.repo {
            let mark_file_path = self.path.join(&repo.name);
            if !mark_file_path.exists() {
                std::fs::File::create(&mark_file_path)?;
                count += 1;
            }
        }
        Ok(count)
    }

    /// name is key
    pub fn into_hash(self) -> HashMap<String, Repo> {
        let mut h = HashMap::new();
        for rs in self.repo {
            h.insert(rs.name.clone(), rs);
        }
        h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env() {
        let root = env!("CARGO_MANIFEST_DIR");
        let mut path = PathBuf::from(root);
        path.push("example");
        std::env::set_var("BUSHI_CONFIG", path);
        Config::new().unwrap();
    }
}
