// playlist.rs - tools for mananging playlists and queuing for the next and previous operations
use crate::Track;

#[derive(Default)]
pub struct PlayList {
    tracks: Vec<Track>,
    ptr: usize,
}

impl PlayList {
    pub fn queue(&mut self, track: Track) {
        // Add song onto the end of the playlist
        self.tracks.push(track);
    }

    pub fn queue_next(&mut self, track: Track) {
        // Add song to play immediately after the current one
        self.tracks.insert(self.ptr + 1, track);
    }

    pub fn play(&mut self, track: Track) -> Option<Track> {
        // Immediately add song and start playing it
        if self.tracks.is_empty() {
            self.queue(track);
            self.current()
        } else {
            self.queue_next(track);
            self.next()
        }
    }

    pub fn set(&mut self, ptr: usize, tracks: Vec<Track>) {
        // Insert a custom playlist to use, as well as an index to start from
        self.ptr = ptr;
        self.tracks = tracks;
    }

    pub fn clear(&mut self) {
        // Clear the playlist
        self.tracks.clear();
        self.ptr = 0;
    }

    pub fn next(&mut self) -> Option<Track> {
        // Switch to the next track in the queue
        if self.ptr + 1 >= self.tracks.len() {
            None
        } else {
            self.ptr += 1;
            self.current()
        }
    }

    pub fn previous(&mut self) -> Option<Track> {
        // Switch to the previously played track
        if self.ptr > 0 {
            self.ptr -= 1;
            self.current()
        } else {
            None
        }
    }

    pub fn current(&mut self) -> Option<Track> {
        // Get the currently playing track
        Some(self.tracks.get(self.ptr)?.clone())
    }

    pub fn move_down(&mut self, ptr: usize) {
        // Move a particular track downwards
        self.tracks.swap(ptr, ptr + 1);
    }

    pub fn move_up(&mut self, ptr: usize) {
        // Move a particular track upwards
        self.tracks.swap(ptr, ptr.saturating_sub(1));
    }

    pub fn move_next(&mut self, ptr: usize) {
        // Move a particular song in this queue to play next
        let track = self.tracks.remove(ptr);
        self.queue_next(track);
    }

    pub fn view(&mut self) -> String {
        let mut result = String::new();
        for (c, track) in self.tracks.iter().enumerate() {
            result.push_str(&format!(
                "{}{}\n",
                if c == self.ptr { "-> " } else { "   " },
                track.format()
            ));
        }
        result
    }

    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }
}
