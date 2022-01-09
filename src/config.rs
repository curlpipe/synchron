use crate::util::expand_path;
use serde::{Deserialize, Serialize};

const DEFAULT: &str = include_str!("../synchron.ron");

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub prompt: String,
}

impl Config {
    pub fn open() -> Self {
        if let Some(config) = attempt_open("~/.config/synchron.ron") {
            // Attempt opening config file in ~/.config directory
            ron::from_str(&config).expect("Invalid config file format!")
        } else if let Some(config) = attempt_open("./synchron.ron") {
            // Attempt opening config file from current working directory
            ron::from_str(&config).expect("Invalid config file format!")
        } else {
            // Use embedded config
            println!("Note: using default config");
            ron::from_str(DEFAULT).expect("Invalid config file format!")
        }
    }
}

pub fn attempt_open(path: &str) -> Option<String> {
    let path = expand_path(path)?;
    std::fs::read_to_string(path).ok()
}
