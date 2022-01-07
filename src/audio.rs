// audio.rs - handling music playback
use gstreamer::prelude::*;
use gstreamer::ClockTime;
use gstreamer_player::{Player, PlayerGMainContextSignalDispatcher, PlayerSignalDispatcher};
use id3::Tag;
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
    pub position: i64,
    pub track: Track,
}

// Main manager struct that handles everything
pub struct Manager {
    player: Player,
    pub metadata: Arc<Mutex<Metadata>>,
    pub update_transmit: Sender<()>,
    pub mpris: Receiver<crate::mpris::Event>,
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
            player,
            // Default placeholder values
            metadata: Arc::new(Mutex::new(Metadata {
                playback_status: PlaybackStatus::Stopped,
                loop_status: LoopStatus::None,
                shuffle_status: false,
                volume: 1.0,
                position: 0,
                track: Track::default(),
            })),
            mpris: rx,
            update_transmit: tx2,
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

    pub fn load(&mut self, track: Track) {
        // Load a track into this player
        let mut md = self.metadata.lock().unwrap();
        md.playback_status = PlaybackStatus::Stopped;
        self.player.set_uri(&track.path);
        md.track = track;
        self.update();
    }

    pub fn play(&mut self) {
        // Play the current track
        let mut md = self.metadata.lock().unwrap();
        md.playback_status = PlaybackStatus::Playing;
        self.player.play();
        self.update();
    }

    pub fn pause(&mut self) {
        // Pause the current track
        let mut md = self.metadata.lock().unwrap();
        md.playback_status = PlaybackStatus::Paused;
        self.player.pause();
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
        self.update();
    }

    pub fn set_loop(&mut self, s: LoopStatus) {
        // Set the loop status
        let mut md = self.metadata.lock().unwrap();
        md.loop_status = s;
        self.update();
    }

    pub fn set_shuffle(&mut self, s: bool) {
        // Set the shuffle status
        let mut md = self.metadata.lock().unwrap();
        md.shuffle_status = s;
        self.update();
    }

    pub fn seek(&mut self, forwards: bool, s: Duration) {
        // Perform a seek operation
        let (mut position, duration, _) = self.get_position();
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

    pub fn set_volume(&mut self, v: f64) {
        // Set the volume of the player
        let mut md = self.metadata.lock().unwrap();
        md.volume = v;
        self.player.set_volume(v);
        self.update();
    }

    pub fn set_position(&mut self, p: i64) {
        // Set the position of the player
        let (_, duration, _) = self.get_position();
        let p = p.try_into().unwrap();
        if p > duration {
            return;
        }
        self.player.seek(ClockTime::from_seconds(p));
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn get_position(&mut self) -> (u64, u64, f64) {
        // Work out the current position of the player
        let time_pos = match self.player.position() {
            Some(t) => ClockTime::seconds(t),
            None => 0_u64,
        };
        // Update the position for mpris to read
        self.metadata.lock().unwrap().position = time_pos.try_into().unwrap_or(0);
        // Work out the duration of the current track
        let duration = match self.player.duration() {
            Some(d) => ClockTime::seconds(d),
            None => 0_u64,
        };
        // Return above values, and calculate the percentage way through
        (time_pos, duration, time_pos as f64 / (duration as f64))
    }

    pub fn metadata(&mut self) -> String {
        // Return the formatted metadata information
        self.metadata.lock().unwrap().track.metadata()
    }

    pub fn update(&self) {
        // Send the update signal for mpris to update it's values
        self.update_transmit.send(()).unwrap();
    }
}

// Track struct to handle file reading, and tag extraction
#[derive(Debug, Default, Clone)]
pub struct Track {
    pub path: String,
    pub tag: Tag,
}

impl Track {
    pub fn load(path: &str) -> Self {
        // Expand provided path, read the tags and create new instance
        let path = Track::format_path(path);
        let path = expanduser::expanduser(path).unwrap();
        let path = std::fs::canonicalize(path).expect("File not found");
        let path = path.into_os_string().into_string().unwrap();
        let tag = Tag::read_from_path(&path).unwrap_or_else(|_| Tag::new());
        let path = format!("file://{}", path);
        Self { path, tag }
    }

    pub fn metadata(&self) -> String {
        // Format metadata
        let title = self.tag.title().unwrap_or("[unknown]").to_string();
        let album = self.tag.album().unwrap_or("[unknown]").to_string();
        let artist = self.tag.artist().unwrap_or("[unknown]").to_string();
        let year = self.tag.year().unwrap_or(0).to_string();
        format!(
            "Title: {}\nArtist: {}\nAlbum: {}\nYear: {}",
            title, artist, album, year
        )
    }

    pub fn format_path(path: &str) -> String {
        // Unify the path format
        path.trim_start_matches("file://").to_string()
    }
}
