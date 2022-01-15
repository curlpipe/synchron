// track.rs - for managing track related activities
use crate::util::expand_path;
use id3::Version;
use serde::{Deserialize, Serialize};

// For holding tag information
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Tag {
    pub title: String,
    pub album: String,
    pub artist: String,
    pub year: String,
}

impl Tag {
    pub fn from_id3(tag: &id3::Tag) -> Self {
        // Load from id3 tag
        Self {
            title: tag.title().unwrap_or("[unknown]").to_string(),
            album: tag.album().unwrap_or("[unknown]").to_string(),
            artist: tag.artist().unwrap_or("[unknown]").to_string(),
            year: tag.year().unwrap_or(0).to_string(),
        }
    }
}

impl Default for Tag {
    fn default() -> Self {
        // Default value for a tag
        Self {
            title: "[unknown]".to_string(),
            album: "[unknown]".to_string(),
            artist: "[unknown]".to_string(),
            year: "0".to_string(),
        }
    }
}

// Track struct to handle file reading, and tag extraction
#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq)]
pub struct Track {
    pub path: String,
    pub tag: Tag,
}

impl Track {
    pub fn load(path: &str) -> Self {
        // Expand provided path, read the tags and create new instance
        let path = Track::format_path(path);
        let path = expand_path(&path).expect("File not found");
        let tag = id3::Tag::read_from_path(&path).unwrap_or_else(|_| id3::Tag::new());
        let path = format!("file://{}", path);
        Self {
            path,
            tag: Tag::from_id3(&tag),
        }
    }

    pub fn set_title(&mut self, title: &str) {
        // Set the title of this track
        let path = Track::format_path(&self.path);
        if let Ok(mut tag) = id3::Tag::read_from_path(&path) {
            tag.set_title(title);
            self.tag.title = title.to_string();
            tag.write_to_path(path, Version::Id3v24).ok();
        }
    }

    pub fn set_album(&mut self, album: &str) {
        // Set the title of this track
        let path = Track::format_path(&self.path);
        if let Ok(mut tag) = id3::Tag::read_from_path(&path) {
            tag.set_album(album);
            self.tag.album = album.to_string();
            tag.write_to_path(path, Version::Id3v24).ok();
        }
    }

    pub fn set_artist(&mut self, artist: &str) {
        // Set the title of this track
        let path = Track::format_path(&self.path);
        if let Ok(mut tag) = id3::Tag::read_from_path(&path) {
            tag.set_artist(artist);
            self.tag.artist = artist.to_string();
            tag.write_to_path(path, Version::Id3v24).ok();
        }
    }

    pub fn set_year(&mut self, year: &str) {
        // Set the title of this track
        let path = Track::format_path(&self.path);
        if let Ok(mut tag) = id3::Tag::read_from_path(&path) {
            tag.set_year(year.parse().unwrap_or(0));
            self.tag.year = year.to_string();
            tag.write_to_path(path, Version::Id3v24).ok();
        }
    }

    pub fn update(&mut self) {
        let path = Track::format_path(&self.path);
        if let Ok(tag) = id3::Tag::read_from_path(&path) {
            self.tag = Tag::from_id3(&tag);
        }
    }

    pub fn format_path(path: &str) -> String {
        // Unify the path format
        path.trim_start_matches("file://").to_string()
    }

    pub fn format_elements(&self) -> (String, &String, &String, &String, &String) {
        let tag = &self.tag;
        (
            Track::format_path(&self.path),
            &tag.title,
            &tag.album,
            &tag.artist,
            &tag.year,
        )
    }

    pub fn format(&self) -> String {
        let (path, title, album, artist, year) = self.format_elements();
        format!("{} | {} | {} | {} | {}", path, title, album, artist, year)
    }
}
