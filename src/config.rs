// config.rs - manage config file and databases
use crate::track::Track;
use crate::util::{attempt_open, expand_path};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Default configuration and database formats
const DEFAULT_CONFIG: &str = include_str!("../synchron.ron");
const DEFAULT_DATABASE: &str = include_str!("../database.ron");
// Thread pulse time for rendering, and dbus actions.
// Lower = Quicker reaction times, worse performance
// Higher = Slower reaction times, better performance
pub const PULSE: u64 = 200;

#[derive(Debug, Deserialize, Serialize)]
pub enum Pane {
    SimpleLibrary,
    SortedLibrary,
    Files,
    Empty,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub prompt: String,
    pub panes: HashMap<u8, Pane>,
    pub open_on_pane: u8,
    pub indicators: HashMap<String, String>,
    pub show_hidden_files: bool,
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
            ron::from_str(DEFAULT_CONFIG).expect("Invalid config file format!")
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Database {
    pub tracks: HashMap<usize, Track>,
    pub playlists: HashMap<String, Vec<usize>>,
    pub display: Vec<usize>,
}

impl Database {
    pub fn open() -> Self {
        // Attempt to open the database
        let path_base =
            expand_path("~/.local/share").unwrap_or_else(|| "~/.local/share".to_string());
        std::fs::create_dir_all(format!("{}/synchron/", path_base)).ok();
        let path_full = format!("{}/synchron/database.ron", path_base);
        if std::path::Path::new(&path_full).exists() {
            // File exists
            if let Some(database) = attempt_open("~/.local/share/synchron/database.ron") {
                // Database read sucessfully
                ron::from_str(&database).expect("Database is corrupted")
            } else {
                // Failed to read database, use empty one
                println!("Note: failed to open database, using empty database");
                ron::from_str(DEFAULT_DATABASE).expect("Database is corrupted")
            }
        } else {
            // File doesn't exist, attempt to write an empty one
            println!("Note: Database not detected, creating empty database");
            if std::fs::write(&path_full, DEFAULT_DATABASE).is_err() {
                // Failed to create database, display error
                println!("ERROR: Failed to create database, using empty database");
            }
            // Read in an empty database
            ron::from_str(DEFAULT_DATABASE).expect("Database is corrupted")
        }
    }

    pub fn write(&self) {
        let path_base =
            expand_path("~/.local/share").unwrap_or_else(|| "~/.local/share".to_string());
        std::fs::create_dir_all(format!("{}/synchron/", path_base)).ok();
        let path_full = format!("{}/synchron/database.ron", path_base);
        if !std::path::Path::new(&path_full).exists() {
            println!("Warning: Database not found, these changes will not be saved");
            return;
        }
        if let Ok(write) = ron::ser::to_string(self) {
            if std::fs::write(path_full, write).is_err() {
                println!("ERROR: Failed to write to disk");
            }
        }
    }
}
