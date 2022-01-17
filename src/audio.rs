// audio.rs - handling music playback
use crate::config::{Config, Database};
use crate::playlist::PlayList;
use crate::track::{Tag, Track};
use gstreamer::prelude::*;
use gstreamer::ClockTime;
use gstreamer_player::{Player, PlayerGMainContextSignalDispatcher, PlayerSignalDispatcher};
use std::sync::{
    mpsc::{self, Receiver, Sender},
    Arc, Mutex,
};
use std::time::Duration;

// Represents playback status
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaybackStatus {
    Playing,
    Paused,
    Stopped,
}

// Represents loop status
#[derive(Debug, Clone, Copy)]
pub enum LoopStatus {
    None,
    Track,
    Playlist,
}

// Stores metadata to be transmitted between threads
#[derive(Debug, Clone)]
pub struct Metadata {
    pub playback_status: PlaybackStatus,
    pub loop_status: LoopStatus,
    pub shuffle_status: bool,
    pub volume: f64,
    pub position: (u64, u64, f64),
    pub tag: Tag,
}

// Main manager struct that handles everything
pub struct Manager {
    pub player: Player,
    pub playlist: PlayList,
    pub metadata: Arc<Mutex<Metadata>>,
    pub update_transmit: Sender<()>,
    pub mpris: Receiver<crate::mpris::Event>,
    pub config: Config,
    pub database: Database,
    // TODO: Replace use of channels with mutexes on this variable.
    pub updated: bool,
}

impl Manager {
    pub fn new() -> Self {
        // Initiate gstreamer player
        gstreamer::init().unwrap();
        let dispatcher = PlayerGMainContextSignalDispatcher::new(None);
        let player = Player::new(None, Some(&dispatcher.upcast::<PlayerSignalDispatcher>()));
        // Set up channel to recieve and send events
        let (_, rx) = mpsc::sync_channel(32);
        // Placeholder channel
        let (tx2, _) = mpsc::channel();
        // Initiate player
        Self {
            // Create player
            player,
            // Initialise an empty playlist
            playlist: PlayList::default(),
            // Default placeholder values
            metadata: Arc::new(Mutex::new(Metadata {
                playback_status: PlaybackStatus::Stopped,
                loop_status: LoopStatus::None,
                shuffle_status: false,
                volume: 1.0,
                position: (0, 0, 0.0),
                tag: Tag::default(),
            })),
            // Add in mpris information channels
            mpris: rx,
            update_transmit: tx2,
            // Load in config file and library database
            config: Config::open(),
            database: Database::open(),
            // Updated flag for UI
            updated: false,
        }
    }

    pub fn init(&mut self) {
        // Initialise this manager
        self.player.set_volume(1.0);
        // Set up channels
        let (tx, rx) = mpsc::sync_channel(32);
        let (tx2, rx2) = mpsc::channel();
        self.update_transmit = tx2;
        self.mpris = rx;
        // Event handler
        let ev = Arc::new(Mutex::new(move |event: crate::mpris::Event| {
            tx.send(event).ok();
        }));
        // Spawn mpris thread
        let md = self.metadata.clone();
        std::thread::spawn(move || crate::mpris::connect(ev, &md, &rx2));
    }

    pub fn open(&mut self, track: Track) {
        // If the track is already in the library, load it, otherwise, add it and then load it
        let mut found = None;
        for (id, value) in &self.database.tracks {
            if value == &track {
                found = Some(*id);
                break;
            }
        }
        if let Some(id) = found {
            self.load(id);
        } else {
            let idx = self.add_library(track);
            self.load(idx);
        }
    }

    pub fn load(&mut self, id: usize) {
        // Load a track into this player
        if let Some(track) = self.database.tracks.get(&id) {
            let mut md = self.metadata.lock().unwrap();
            md.playback_status = PlaybackStatus::Stopped;
            md.tag = track.tag.clone();
            self.playlist.play(track.clone(), id);
            self.player
                .set_uri(self.playlist.current().unwrap().path.as_str());
            std::mem::drop(md);
            self.update();
        } else {
            println!("ERROR: Track ID out of range: {}", id);
        }
    }

    pub fn load_playlist(&mut self, playlist: &str) {
        // Load a playlist in
        let mut md = self.metadata.lock().unwrap();
        if let Some(load) = self.database.playlists.get(playlist) {
            let mut playlist = vec![];
            for id in load {
                playlist.push(self.database.tracks[id].clone());
            }
            self.playlist.set(0, playlist, load.clone());
            md.playback_status = PlaybackStatus::Stopped;
            if self.playlist.is_empty() {
                md.tag = Tag::default();
                self.player.set_uri("");
            } else {
                md.tag = self.playlist.current().unwrap().tag;
                self.player
                    .set_uri(self.playlist.current().unwrap().path.as_str());
            }
            std::mem::drop(md);
            self.update();
        } else {
            println!("ERROR: Couldn't find playlist: {}", playlist);
        }
    }

    pub fn new_playlist(&mut self, name: &str) {
        // Create a new playlist
        self.database.playlists.insert(name.to_string(), vec![]);
        self.database.write();
    }

    pub fn list_playlist(&mut self, name: &str) -> String {
        // List a playlist
        let mut result = format!("{}:\n", name);
        if let Some(load) = self.database.playlists.get(name) {
            for id in load {
                result.push_str(&format!("{}\n", self.database.tracks[id].format()));
            }
        } else {
            result = format!("ERROR: Couldn't find playlist: {}", name);
        }
        result
    }

    pub fn list_playlists(&self) -> String {
        // List all the playlists
        let mut result = String::new();
        for i in self.database.playlists.keys() {
            result.push_str(&format!("{}\n", i));
        }
        result
    }

    pub fn rename_playlist(&mut self, old: &str, new: &str) {
        // Rename a playlist to something else
        if let Some(val) = self.database.playlists.remove(old) {
            self.database.playlists.insert(new.to_string(), val);
            self.database.write();
        } else {
            println!("ERROR: Couldn't find playlist: {}", old);
        }
    }

    pub fn delete_playlist(&mut self, name: &str) {
        // Delete a playlist
        if self.database.playlists.remove(name).is_some() {
            self.database.write();
        } else {
            println!("ERROR: Couldn't find playlist: {}", name);
        }
    }

    pub fn add_to_playlist(&mut self, playlist: &str, track: usize) {
        if let Some(load) = self.database.playlists.get_mut(playlist) {
            if self.database.tracks.len() > track {
                load.push(track);
                self.database.write();
            } else {
                println!("ERROR: Track ID out of range: {}", track);
            }
        } else {
            println!("ERROR: Couldn't find playlist: {}", playlist);
        }
    }

    pub fn remove_from_playlist(&mut self, playlist: &str, idx: usize) {
        if let Some(load) = self.database.playlists.get_mut(playlist) {
            load.remove(idx);
            self.database.write();
        } else {
            println!("ERROR: Couldn't find playlist: {}", playlist);
        }
    }

    pub fn queue(&mut self, id: usize) {
        // Queue a track
        if let Some(track) = self.database.tracks.get(&id) {
            self.playlist.queue(track.clone(), id);
        }
    }

    pub fn clear_queue(&mut self) {
        // Clear the queue and stop playback
        self.playlist.clear();
        self.stop();
    }

    pub fn play(&mut self) {
        // Play the current track
        if !self.playlist.is_empty() {
            let mut md = self.metadata.lock().unwrap();
            if md.playback_status == PlaybackStatus::Stopped {
                self.player.stop();
            }
            md.playback_status = PlaybackStatus::Playing;
            self.player.play();
            std::mem::drop(md);
            self.update();
        }
    }

    pub fn pause(&mut self) {
        // Pause the current track
        let mut md = self.metadata.lock().unwrap();
        md.playback_status = PlaybackStatus::Paused;
        self.player.pause();
        std::mem::drop(md);
        self.update();
    }

    pub fn play_pause(&mut self) {
        // Toggle play or pause on the track
        let status = self.metadata.lock().unwrap().playback_status;
        match status {
            PlaybackStatus::Paused | PlaybackStatus::Stopped => self.play(),
            PlaybackStatus::Playing => self.pause(),
        }
    }

    pub fn stop(&mut self) {
        // Stop the currently playing track
        let mut md = self.metadata.lock().unwrap();
        md.playback_status = PlaybackStatus::Stopped;
        self.player.stop();
        std::mem::drop(md);
        self.update();
    }

    pub fn next(&mut self) -> Option<()> {
        // Move to the next track
        let next = self.playlist.next()?;
        self.player.set_uri(&next.path);
        self.metadata.lock().unwrap().tag = next.tag;
        self.play();
        self.update();
        Some(())
    }

    pub fn previous(&mut self) -> Option<()> {
        // Move to the previous track
        let previous = self.playlist.previous()?;
        self.player.set_uri(&previous.path);
        self.metadata.lock().unwrap().tag = previous.tag;
        self.play();
        self.update();
        Some(())
    }

    pub fn set_loop(&mut self, s: LoopStatus) {
        // Set the loop status
        let mut md = self.metadata.lock().unwrap();
        md.loop_status = s;
        std::mem::drop(md);
        self.update();
    }

    pub fn cycle_loop(&mut self) {
        // Cycle through the loop statuses
        let mut md = self.metadata.lock().unwrap();
        md.loop_status = match md.loop_status {
            LoopStatus::None => LoopStatus::Track,
            LoopStatus::Track => LoopStatus::Playlist,
            LoopStatus::Playlist => LoopStatus::None,
        };
        std::mem::drop(md);
        self.update();
    }

    pub fn set_shuffle(&mut self, s: bool) {
        // Set the shuffle status
        let mut md = self.metadata.lock().unwrap();
        md.shuffle_status = s;
        std::mem::drop(md);
        self.update();
    }

    pub fn cycle_shuffle(&mut self) {
        // Toggle the shuffle option
        let mut md = self.metadata.lock().unwrap();
        md.shuffle_status = !md.shuffle_status;
        std::mem::drop(md);
        self.update();
    }

    pub fn seek(&mut self, forwards: bool, s: Duration) {
        // Perform a seek operation
        if self.metadata.lock().unwrap().playback_status != PlaybackStatus::Stopped {
            // Player is not stopped and ready to be seeked
            if let Some((mut position, duration, _)) = self.get_position() {
                // Update position
                position = if forwards {
                    position + s.as_secs()
                } else {
                    position.saturating_sub(s.as_secs())
                };
                if position > duration {
                    position = duration;
                }
                self.player.seek(ClockTime::from_seconds(position));
            }
        }
    }

    pub fn set_volume(&mut self, v: f64) {
        // Set the volume of the player
        if v >= 0.0 {
            let mut md = self.metadata.lock().unwrap();
            md.volume = v;
            self.player.set_volume(v);
            std::mem::drop(md);
            self.update();
        }
    }

    pub fn toggle_mute(&mut self) {
        // Toggle the mute option
        let md = self.metadata.lock().unwrap();
        if self.player.volume() == 0.0 {
            self.player.set_volume(md.volume);
        } else {
            self.player.set_volume(0.0);
        }
        std::mem::drop(md);
        self.update();
    }

    pub fn set_position(&mut self, p: i64) {
        // Set the position of the player
        if let Some((_, duration, _)) = self.get_position() {
            let p = p.try_into().unwrap();
            if p > duration {
                return;
            }
            self.player.seek(ClockTime::from_seconds(p));
        }
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn get_position(&self) -> Option<(u64, u64, f64)> {
        // Work out the current position of the player
        let time_pos = ClockTime::seconds(self.player.position()?);
        // Work out the duration of the current track
        let duration = ClockTime::seconds(self.player.duration()?);
        // Tupleize above values, and calculate the percentage way through
        let data = (time_pos, duration, time_pos as f64 / (duration as f64));
        // Update the position for mpris to read
        self.metadata.lock().unwrap().position = data;
        // Return formed values
        Some(data)
    }

    pub fn list_library(&self) -> String {
        // List all the tracks in the library
        let mut keys: Vec<usize> = self.database.tracks.keys().copied().collect();
        keys.sort_unstable();
        let mut result = String::new();
        for id in keys {
            result.push_str(&format!("{}: {}\n", id, self.database.tracks[&id].format()));
        }
        result
    }

    pub fn add_library(&mut self, track: Track) -> usize {
        // Add a track to the library
        let mut keys: Vec<usize> = self.database.tracks.keys().copied().collect();
        keys.sort_unstable();
        let mut i = 0;
        let mut result = None;
        for k in &keys {
            if i != *k {
                result = Some(i);
                break;
            }
            i += 1;
        }
        let result = result.unwrap_or(i);
        self.database.tracks.insert(result, track);
        self.database.write();
        result
    }

    pub fn remove_library(&mut self, id: usize) {
        // Remove a track from the library
        self.database.tracks.remove(&id);
        for values in self.database.playlists.values_mut() {
            if let Some(idx) = values.iter().position(|x| *x == id) {
                values.remove(idx);
            }
        }
        self.database.write();
    }

    pub fn set_title(&mut self, id: usize, new: &str) {
        // Set the title of a track
        if let Some(track) = self.database.tracks.get_mut(&id) {
            track.set_title(new);
            self.database.write();
        } else {
            println!("ERROR: Track ID out of range: {}", id);
        }
    }

    pub fn set_album(&mut self, id: usize, new: &str) {
        // Set the album of a track
        if let Some(track) = self.database.tracks.get_mut(&id) {
            track.set_album(new);
            self.database.write();
        } else {
            println!("ERROR: Track ID out of range: {}", id);
        }
    }

    pub fn set_artist(&mut self, id: usize, new: &str) {
        // Set the artist of a track
        if let Some(track) = self.database.tracks.get_mut(&id) {
            track.set_artist(new);
            self.database.write();
        } else {
            println!("ERROR: Track ID out of range: {}", id);
        }
    }

    pub fn set_year(&mut self, id: usize, new: &str) {
        // Set the year of a track
        if let Some(track) = self.database.tracks.get_mut(&id) {
            track.set_year(new);
            self.database.write();
        } else {
            println!("ERROR: Track ID out of range: {}", id);
        }
    }

    pub fn update_tag(&mut self, id: usize) {
        // Reread the tags of a track
        if let Some(track) = self.database.tracks.get_mut(&id) {
            track.update();
            self.database.write();
        } else {
            println!("ERROR: Track ID out of range: {}", id);
        }
    }

    pub fn view_track(&mut self, id: usize) {
        // View track metadata
        if let Some(track) = self.database.tracks.get_mut(&id) {
            println!("{}", track.format());
        } else {
            println!("ERROR: Track ID out of range: {}", id);
        }
    }

    pub fn update(&mut self) {
        // Send the update signal for mpris to update it's values
        self.updated = true;
        self.update_transmit.send(()).unwrap();
    }
}
