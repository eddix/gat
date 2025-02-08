use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize)]
pub struct Config {
    pub repository: Vec<Repository>,
}

#[derive(Deserialize)]
pub struct Repository {
    pub name: Option<String>,
    pub location: String,
    pub description: Option<String>,
}

pub fn from_file(file: &str) -> Result<Config> {
    let config: Config = toml::from_str(&std::fs::read_to_string(file)?)?;

    Ok(config)
}

impl Repository {
    pub fn name(&self) -> &str {
        match &self.name {
            None => Path::new(self.location.as_str())
                .file_name()
                .unwrap()
                .to_str()
                .unwrap(),
            Some(name) => name.as_str(),
        }
    }
}
