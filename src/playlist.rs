// playlist.rs - tools for mananging playlists and queuing for the next and previous operations
use crate::Track;

#[derive(Default)]
pub struct PlayList {
    tracks: Vec<Track>,
    pub ids: Vec<usize>,
    pub ptr: Option<usize>,
    pub name: Option<String>,
}

impl PlayList {
    pub fn queue(&mut self, track: Track, id: usize) {
        // Add song onto the end of the playlist
        self.tracks.push(track);
        self.ids.push(id);
    }

    pub fn queue_next(&mut self, track: Track, id: usize) {
        // Add song to play immediately after the current one
        self.tracks.insert(self.get_ptr() + 1, track);
        self.ids.insert(self.get_ptr() + 1, id);
    }

    pub fn play(&mut self, track: Track, id: usize) -> Option<Track> {
        // Immediately add song and start playing it
        if !self.is_ready() {
            self.ptr = Some(0);
        }
        if self.tracks.is_empty() {
            self.queue(track, id);
            self.current()
        } else {
            self.queue_next(track, id);
            self.next()
        }
    }

    pub fn set(&mut self, ptr: usize, tracks: Vec<Track>, ids: Vec<usize>) {
        // Insert a custom playlist to use, as well as an index to start from
        self.ptr = Some(ptr);
        self.tracks = tracks;
        self.ids = ids;
    }

    pub fn clear(&mut self) {
        // Clear the playlist
        self.tracks.clear();
        self.ids.clear();
        self.ptr = Some(0);
    }

    pub fn next(&mut self) -> Option<Track> {
        // Switch to the next track in the queue
        if self.ptr? + 1 >= self.tracks.len() {
            None
        } else {
            self.ptr = Some(self.ptr? + 1);
            self.current()
        }
    }

    pub fn previous(&mut self) -> Option<Track> {
        // Switch to the previously played track
        if self.ptr? > 0 {
            self.ptr = Some(self.ptr? - 1);
            self.current()
        } else {
            None
        }
    }

    pub fn current_id(&self) -> Option<usize> {
        // Get the currently playing track ID
        Some(*self.ids.get(self.ptr?)?)
    }

    pub fn current(&self) -> Option<Track> {
        // Get the currently playing track
        if !self.is_ready() {
            return None;
        }
        Some(self.tracks.get(self.ptr?)?.clone())
    }

    pub fn is_ready(&self) -> bool {
        self.ptr.is_some()
    }

    pub fn get_ptr(&self) -> usize {
        self.ptr.unwrap()
    }

    pub fn move_down(&mut self, ptr: usize) {
        // Move a particular track downwards
        self.tracks.swap(ptr, ptr + 1);
        self.ids.swap(ptr, ptr + 1);
        if ptr == self.get_ptr() {
            self.ptr = Some(self.get_ptr() + 1);
        }
    }

    pub fn move_up(&mut self, ptr: usize) {
        // Move a particular track upwards
        self.tracks.swap(ptr, ptr.saturating_sub(1));
        self.ids.swap(ptr, ptr.saturating_sub(1));
        if ptr == self.get_ptr() {
            self.ptr = Some(self.get_ptr().saturating_sub(1));
        }
    }

    pub fn move_next(&mut self, ptr: usize) {
        // Move a particular song in this queue to play next
        let track = self.tracks.remove(ptr);
        let id = self.ids.remove(ptr);
        self.queue_next(track, id);
    }

    pub fn view(&mut self) -> String {
        let mut result = String::new();
        for (c, track) in self.tracks.iter().enumerate() {
            result.push_str(&format!(
                "{}{}\n",
                if c == self.get_ptr() { "-> " } else { "   " },
                track.format()
            ));
        }
        result
    }

    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }
}
