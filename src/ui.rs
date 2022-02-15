// ui.rs - controls and renders the TUI
use crate::audio::{LoopStatus, Manager, PlaybackStatus};
use crate::config::{Pane, PULSE};
use crate::track::Track;
use crate::util::{
    align_sides, artist_tracks, expand_path, form_library_tree, format_artist_track,
    format_playlist, format_table, is_file, list_dir, pad_table, timefmt,
};
pub use crossterm::{
    cursor,
    event::{self, Event, KeyCode as KCode, KeyEvent, KeyModifiers as KMod},
    execute, queue,
    style::{self, Color, Print, SetBackgroundColor as SetBg, SetForegroundColor as SetFg},
    terminal::{self, ClearType},
    Command, Result,
};
use std::collections::HashMap;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;

type OptionList = Option<Vec<String>>;
type TrackList = (Option<Vec<usize>>, OptionList);
type SortedList = Option<Vec<String>>;
type FileList = OptionList;

pub struct Size {
    width: u16,
    height: u16,
}

impl Size {
    pub fn screen() -> Result<Self> {
        // Form a Size struct from the screen size
        let (width, height) = terminal::size()?;
        Ok(Self { width, height })
    }
}

#[derive(PartialEq)]
pub enum State {
    Library {
        selection: usize,
    },
    Files {
        selection: usize,
        dir: String,
        list: Vec<String>,
    },
    SortedLibrary {
        depth: u8,
        artist: String,
        track: HashMap<String, usize>,
    },
    Playlists {
        depth: u8,
        playlist: String,
        track: HashMap<String, usize>,
    },
    Empty,
}

impl State {
    pub fn is_library(&self) -> bool {
        matches!(self, Self::Library { .. })
    }

    pub fn is_files(&self) -> bool {
        matches!(self, Self::Files { .. })
    }

    pub fn is_sorted_library(&self) -> bool {
        matches!(self, Self::SortedLibrary { .. })
    }

    pub fn is_playlists(&self) -> bool {
        matches!(self, Self::Playlists { .. })
    }

    pub fn get_selection(&self) -> usize {
        match self {
            Self::Library { selection, .. } => *selection,
            Self::Files { selection, .. } => *selection,
            _ => unreachable!(),
        }
    }
}

pub struct Ui {
    stdout: std::io::Stdout,
    mgmt: Arc<Mutex<Manager>>,
    states: HashMap<u8, State>,
    ptr: u8,
    play_ptr: u8,
    size: Size,
    active: bool,
    library_updated: bool,
}

impl Ui {
    pub fn new(m: Arc<Mutex<Manager>>) -> Result<Self> {
        // Create new UI
        let mgmt = m.lock().unwrap();
        // Create track pointers for artists
        let mut track = HashMap::new();
        let first_artist = mgmt
            .library_tree
            .keys()
            .next()
            .and_then(|x| Some(x.to_string()))
            .unwrap_or_else(|| "".to_string());
        for artist in mgmt.library_tree.keys() {
            track.insert(artist.clone(), 0);
        }
        // Create initial playlist data
        let mut playlist_ptrs = HashMap::new();
        for playlist in mgmt.database.playlists.keys() {
            playlist_ptrs.insert(playlist.to_string(), 0);
        }
        let playlist = mgmt
            .database
            .display
            .playlists
            .get(0)
            .and_then(|n| Some(n.to_string()))
            .unwrap_or_else(|| "".to_string());
        // Set up states
        let mut states = HashMap::default();
        for (key, pane) in &mgmt.config.panes {
            states.insert(
                *key,
                match pane {
                    Pane::SimpleLibrary => State::Library { selection: 0 },
                    Pane::SortedLibrary => State::SortedLibrary {
                        depth: 0,
                        track: track.clone(),
                        artist: first_artist.clone(),
                    },
                    Pane::Files => {
                        let dir = expand_path("~/").unwrap_or_else(|| ".".to_string());
                        State::Files {
                            selection: 0,
                            list: list_dir(&dir, !mgmt.config.show_hidden_files),
                            dir,
                        }
                    }
                    Pane::Playlists => State::Playlists {
                        depth: 0,
                        track: playlist_ptrs.clone(),
                        playlist: playlist.to_string(),
                    },
                    Pane::Empty => State::Empty,
                },
            );
        }
        let ptr = mgmt.config.open_on_pane;
        std::mem::drop(mgmt);
        // Form struct
        Ok(Self {
            stdout: std::io::stdout(),
            mgmt: m,
            states,
            ptr,
            play_ptr: ptr,
            size: Size::screen()?,
            active: true,
            library_updated: false,
        })
    }

    pub fn init(&mut self) -> Result<()> {
        // Initiate the UI
        execute!(self.stdout, terminal::EnterAlternateScreen, cursor::Hide)?;
        terminal::enable_raw_mode()?;
        Ok(())
    }

    pub fn run(&mut self) -> Result<()> {
        // Run the UI
        self.render()?;
        while self.active {
            let status = get_md!(self.mgmt).playback_status;
            if event::poll(std::time::Duration::from_millis(PULSE))? {
                match event::read()? {
                    Event::Key(k) => self.on_key(k),
                    Event::Resize(width, height) => {
                        self.size = Size { width, height };
                        self.render()?;
                    }
                    Event::Mouse(..) => (),
                }
                self.render()?;
            } else if self.mgmt.lock().unwrap().updated {
                self.mgmt.lock().unwrap().updated = false;
                self.render()?;
            } else if status == PlaybackStatus::Playing {
                // Rerender the status line if playing, to keep up with the position of the song
                let status_idx = self.size.height.saturating_sub(1);
                queue!(
                    self.stdout,
                    cursor::MoveTo(0, status_idx),
                    terminal::Clear(ClearType::CurrentLine)
                )?;
                self.rerender_status()?;
                self.stdout.flush()?;
            }
        }
        Ok(())
    }

    pub fn on_key(&mut self, e: KeyEvent) {
        // Handle key event
        match (e.modifiers, e.code) {
            // Mode switching
            (KMod::NONE, KCode::Char('0')) => self.switch_mode(0),
            (KMod::NONE, KCode::Char('1')) => self.switch_mode(1),
            (KMod::NONE, KCode::Char('2')) => self.switch_mode(2),
            (KMod::NONE, KCode::Char('3')) => self.switch_mode(3),
            (KMod::NONE, KCode::Char('4')) => self.switch_mode(4),
            (KMod::NONE, KCode::Char('5')) => self.switch_mode(5),
            (KMod::NONE, KCode::Char('6')) => self.switch_mode(6),
            (KMod::NONE, KCode::Char('7')) => self.switch_mode(7),
            (KMod::NONE, KCode::Char('8')) => self.switch_mode(8),
            (KMod::NONE, KCode::Char('9')) => self.switch_mode(9),
            // [q] : Quit
            (KMod::NONE, KCode::Char('q')) => self.active = false,
            // [t] : Toggle playback
            (KMod::NONE, KCode::Char('t')) => self.mgmt.lock().unwrap().play_pause(),
            // [x] : Stop playback
            (KMod::NONE, KCode::Char('x')) => self.mgmt.lock().unwrap().stop(),
            // [c] : Play playback
            (KMod::NONE, KCode::Char('c')) => self.mgmt.lock().unwrap().play(),
            // [v] : Pause playback
            (KMod::NONE, KCode::Char('v')) => self.mgmt.lock().unwrap().pause(),
            // [d] : Delete from library / Delete playlist
            (KMod::NONE, KCode::Char('d')) => {
                if self.state().is_playlists() {
                    self.delete_playlist();
                } else {
                    self.remove();
                }
            }
            // [e] : Edit tag of selected song
            (KMod::NONE, KCode::Char('e')) => self.tag_edit().unwrap_or(()),
            // [Enter] : Play selection / Add track to library
            (KMod::NONE, KCode::Enter) => self.select(),
            // [/\] : Move up selection in library
            (KMod::NONE, KCode::Up) => self.selection_up(),
            // [\/] : Move down selection in library
            (KMod::NONE, KCode::Down) => self.selection_down(),
            // [Ctrl] + [\/] : Move selection to top of library
            (KMod::CONTROL, KCode::Up) => self.selection_top(),
            // [Ctrl] + [/\] : Move selection to bottom of library
            (KMod::CONTROL, KCode::Down) => self.selection_bottom(),
            // [Alt] + [\/] : Move track downwards
            (KMod::ALT, KCode::Up) => self.track_up(),
            // [Alt] + [/\] : Move track upwards
            (KMod::ALT, KCode::Down) => self.track_down(),
            // [<] : Seek backward 5 seconds
            (KMod::NONE, KCode::Left) => self
                .mgmt
                .lock()
                .unwrap()
                .seek(false, Duration::from_secs(5)),
            // [>] : Seek forward 5 seconds
            (KMod::NONE, KCode::Right) => {
                self.mgmt.lock().unwrap().seek(true, Duration::from_secs(5));
            }
            // [Ctrl] + [<] : Previous track
            (KMod::CONTROL, KCode::Left) => self.mgmt.lock().unwrap().previous().unwrap_or(()),
            // [Ctrl] + [>] : Next track
            (KMod::CONTROL, KCode::Right) => self.mgmt.lock().unwrap().next().unwrap_or(()),
            // [l] : Toggle loop status
            (KMod::NONE, KCode::Char('l')) => self.mgmt.lock().unwrap().cycle_loop(),
            // [h] : Toggle shuffle status
            (KMod::NONE, KCode::Char('h')) => self.mgmt.lock().unwrap().cycle_shuffle(),
            // [m] : Toggle mute
            (KMod::NONE, KCode::Char('m')) => self.mgmt.lock().unwrap().toggle_mute(),
            // [Shift] + [/\] : Volume up
            (KMod::SHIFT, KCode::Up) => {
                let v = get_md!(self.mgmt).volume;
                self.mgmt.lock().unwrap().set_volume(v + 0.1);
            }
            // [Shift] + [\/] : Volume down
            (KMod::SHIFT, KCode::Down) => {
                let v = get_md!(self.mgmt).volume;
                self.mgmt.lock().unwrap().set_volume(v - 0.1);
            }
            // [Tab] : Recurse deeper into sorted library
            (KMod::NONE, KCode::Tab) => self.deepen(),
            // [a] : Add to playlist
            (KMod::NONE, KCode::Char('a')) => self.add_to_playlist(),
            // [r] : Remove from playlist
            (KMod::NONE, KCode::Char('r')) => self.remove_from_playlist(),
            // [n] : New playlist
            (KMod::NONE, KCode::Char('n')) => self.create_playlist(),
            // [k] : Rename playlist
            (KMod::NONE, KCode::Char('k')) => self.rename_playlist(),
            // [;] or [:] : Command mode
            (KMod::NONE, KCode::Char(':' | ';')) => (),
            // [???] : Do nothing
            _ => (),
        }
    }

    fn create_playlist(&mut self) {
        // Create new playlist
        if let Ok(Some(name)) = self.get_input("Playlist name: ") {
            if name.is_empty() {
                return;
            }
            self.states.iter_mut().for_each(|(_, s)| {
                if let State::Playlists {
                    track, playlist: p, ..
                } = s
                {
                    track.insert(name.clone(), 0);
                    if p.is_empty() {
                        // Fix empty track pointer
                        *p = name.clone();
                    }
                }
            });
            self.mgmt.lock().unwrap().new_playlist(&name);
        }
    }

    fn delete_playlist(&mut self) {
        // Delete playlist
        if let State::Playlists {
            depth: 0, playlist, ..
        } = self.state()
        {
            if playlist.is_empty() {
                return;
            }
            // Confirm user choice
            let playlist = playlist.clone();
            let warning = format!(
                "WARNING: Are you sure you want '{}' to be deleted? (y/n): ",
                playlist
            );
            if let Ok(Some(confirm)) = self.get_input(&warning) {
                if confirm == "y" {
                    // Move selection up
                    self.selection_up();
                    // Delete track pointers from playlist states
                    self.states.iter_mut().for_each(|(_, s)| {
                        if let State::Playlists {
                            track, playlist: p, ..
                        } = s
                        {
                            // Do removal
                            track.remove(&playlist);
                            if self.mgmt.lock().unwrap().database.display.playlists.get(0)
                                == Some(&playlist)
                            {
                                // Pointer needs fixing
                                *p = self
                                    .mgmt
                                    .lock()
                                    .unwrap()
                                    .database
                                    .display
                                    .playlists
                                    .get(1)
                                    .and_then(|x| Some(x.to_string()))
                                    .unwrap_or_else(|| "".to_string());
                            }
                        }
                    });
                    // Do deletion
                    self.mgmt.lock().unwrap().delete_playlist(&playlist);
                }
            }
        }
    }

    fn rename_playlist(&mut self) {
        // Rename playlist
        if let State::Playlists {
            depth: 0, playlist, ..
        } = self.state()
        {
            if playlist.is_empty() {
                return;
            }
            // Get new playlist name
            let playlist = playlist.clone();
            let msg = format!("Rename '{}' to: ", playlist);
            if let Ok(Some(new)) = self.get_input(&msg) {
                if new.is_empty() {
                    return;
                }
                // Rename track pointers
                self.states.iter_mut().for_each(|(_, s)| {
                    if let State::Playlists {
                        track, playlist: p, ..
                    } = s
                    {
                        // Update playlist pointer if necessary
                        if *p == playlist {
                            *p = new.to_string();
                        }
                        let old: usize = track.remove(&playlist).unwrap();
                        track.insert(new.to_string(), old);
                    }
                });
                // Do renaming
                self.mgmt.lock().unwrap().rename_playlist(&playlist, &new);
            }
        }
    }

    fn add_to_playlist(&mut self) {
        // Add song to playlist from simple library pane
        if let Some(id) = self.get_selected_id() {
            // Get the desired playlist that the user wants to add to
            if let Ok(Some(playlist)) = self.get_input("Playlist name: ") {
                // Check the playlist exists
                if self
                    .mgmt
                    .lock()
                    .unwrap()
                    .database
                    .playlists
                    .contains_key(&playlist)
                {
                    self.mgmt.lock().unwrap().add_to_playlist(&playlist, id);
                }
            }
        }
    }

    fn remove_from_playlist(&mut self) {
        let mut fix_selection = false;
        if let State::Playlists {
            playlist,
            track,
            depth,
        } = self.state()
        {
            if playlist.is_empty() {
                return;
            }
            let length = self.mgmt.lock().unwrap().database.playlists[playlist].len();
            if length == 0 {
                return;
            }
            // Differentiate between deleting playlists and deleting tracks from playlists
            if depth == &1 {
                self.mgmt
                    .lock()
                    .unwrap()
                    .remove_from_playlist(playlist, track[playlist]);
            }
            // Determine if selection needs fixing (out of bounds)
            if track[playlist] > length.saturating_sub(2) {
                fix_selection = true;
            }
        }
        if fix_selection {
            self.selection_up();
        }
    }

    fn get_selected_id(&self) -> Option<usize> {
        // Get the track id that is selected (state independent)
        Some(match self.state() {
            State::Library { selection, .. } => {
                self.mgmt.lock().unwrap().database.display.simple[*selection]
            }
            State::SortedLibrary {
                artist,
                track,
                depth: 1,
                ..
            } => artist_tracks(&self.mgmt.lock().unwrap().library_tree, artist)[track[artist]],
            _ => return None,
        })
    }

    fn deepen(&mut self) {
        // Switch focus in the sorted library view
        match self.state_mut() {
            State::SortedLibrary { depth, .. } => {
                if depth == &1 {
                    *depth = 0;
                } else {
                    *depth += 1;
                }
            }
            State::Playlists {
                depth, playlist, ..
            } => {
                if playlist.is_empty() {
                    return;
                }
                if depth == &1 {
                    *depth = 0;
                } else {
                    *depth += 1;
                }
            }
            _ => (),
        }
    }

    fn tag_edit(&mut self) -> Result<()> {
        // Ensure there are available tracks
        if self.mgmt.lock().unwrap().database.tracks.is_empty() {
            return Ok(());
        }
        // If there is enough room...
        if self.size.height > 3 {
            // Get selected track
            if let Some(id) = self.get_selected_id() {
                // Establish tag type to edit
                let mut kind = String::new();
                while !["title", "album", "artist", "year"].contains(&kind.as_str()) {
                    kind = self
                        .get_input("title/album/artist/year: ")?
                        .unwrap_or_else(|| "".to_string());
                    if kind == "" {
                        return Ok(());
                    }
                }
                // Establish new tag value
                if let Some(value) = self.get_input("new value: ")? {
                    // Write tag value
                    match kind.as_str() {
                        "title" => self.mgmt.lock().unwrap().set_title(id, &value),
                        "album" => self.mgmt.lock().unwrap().set_album(id, &value),
                        "artist" => self.mgmt.lock().unwrap().set_artist(id, &value),
                        "year" => self.mgmt.lock().unwrap().set_year(id, &value),
                        _ => unreachable!(),
                    }
                }
            }
        }
        Ok(())
    }

    fn get_input(&mut self, prompt: &str) -> Result<Option<String>> {
        // If too few rows, don't bother doing prompt
        if self.size.height < 3 {
            return Ok(None);
        }
        // Establish empty row at the bottom
        let input_row = self.size.height;
        self.size.height -= 1;
        self.render()?;
        // Get user input
        let mut out = String::new();
        let mut entering = true;
        while entering {
            execute!(
                self.stdout,
                cursor::MoveTo(0, input_row),
                terminal::Clear(ClearType::CurrentLine),
                Print(prompt),
                Print(&out)
            )?;
            // Handle prompt input
            let status = get_md!(self.mgmt).playback_status;
            if event::poll(std::time::Duration::from_millis(PULSE))? {
                match event::read()? {
                    Event::Key(k) => match (k.modifiers, k.code) {
                        (KMod::NONE | KMod::SHIFT, KCode::Char(c)) => out.push(c),
                        (KMod::NONE, KCode::Backspace) => {
                            let _ = out.pop();
                        }
                        (KMod::NONE, KCode::Enter) => {
                            entering = false;
                        }
                        (KMod::NONE, KCode::Esc) => {
                            self.size = Size::screen()?;
                            return Ok(None);
                        }
                        _ => (),
                    },
                    Event::Resize(width, height) => {
                        self.size = Size {
                            width,
                            height: height - 1,
                        };
                        self.render()?;
                    }
                    Event::Mouse(..) => (),
                }
                self.render()?;
            } else if self.mgmt.lock().unwrap().updated {
                self.mgmt.lock().unwrap().updated = false;
                self.render()?;
            } else if status == PlaybackStatus::Playing {
                // Rerender the status line if playing, to keep up with the position of the song
                let status_idx = self.size.height.saturating_sub(1);
                queue!(
                    self.stdout,
                    cursor::MoveTo(0, status_idx),
                    terminal::Clear(ClearType::CurrentLine)
                )?;
                self.rerender_status()?;
                self.stdout.flush()?;
            }
            self.render()?;
        }
        // Reset shifted row
        self.size = Size::screen()?;
        Ok(Some(out))
    }

    fn switch_mode(&mut self, mode: u8) {
        // Switch modes
        if self.states.contains_key(&mode) {
            self.ptr = mode;
        }
    }

    fn state(&self) -> &State {
        // Get the current state
        self.states.get(&self.ptr).unwrap()
    }

    fn state_mut(&mut self) -> &mut State {
        // Get the current state as a mutable reference
        self.states.get_mut(&self.ptr).unwrap()
    }

    fn remove(&mut self) {
        // Ensure there are available tracks
        if self.mgmt.lock().unwrap().database.tracks.is_empty() {
            return;
        }
        // Remove from library
        let mut selection_off = false;
        match self.state() {
            State::Library { selection, .. } => {
                // Get track ID
                if let Some(id) = self.get_selected_id() {
                    let mut mgmt = self.mgmt.lock().unwrap();
                    mgmt.remove_library(id);
                    // Check for selection issues
                    if selection > &mgmt.database.display.simple.len().saturating_sub(2) {
                        selection_off = true;
                    }
                    // Trigger library tree rerender
                    self.library_updated = true;
                }
            }
            State::SortedLibrary {
                depth,
                artist,
                track,
                ..
            } => {
                if *depth == 1 {
                    let mut mgmt = self.mgmt.lock().unwrap();
                    let tracks = artist_tracks(&mgmt.library_tree, artist);
                    // Get track ID
                    let id = tracks[track[artist]];
                    mgmt.remove_library(id);
                    // Check for selection issues
                    if track[artist] > tracks.len().saturating_sub(3) {
                        selection_off = true;
                    }
                    // Trigger library tree rerender
                    self.library_updated = true;
                }
            }
            _ => (),
        }
        // Correct selection issues
        if selection_off {
            self.selection_up();
        }
    }

    fn select(&mut self) {
        // Play the selected track
        match self.state() {
            State::Library { selection, .. } => {
                let mut mgmt = self.mgmt.lock().unwrap();
                mgmt.playlist.name = None;
                // Ensure there are available tracks
                if mgmt.database.tracks.is_empty() {
                    return;
                }
                let lookup = mgmt.database.display.simple.clone();
                let tracks = lookup
                    .iter()
                    .map(|x| mgmt.database.tracks[x].clone())
                    .collect();
                let id = lookup[*selection];
                mgmt.load(id);
                mgmt.playlist.set(*selection, tracks, lookup);
                self.play_ptr = self.ptr;
                mgmt.play();
            }
            State::SortedLibrary { artist, track, .. } => {
                let mut mgmt = self.mgmt.lock().unwrap();
                mgmt.playlist.name = None;
                let lookup = artist_tracks(&mgmt.library_tree, artist);
                let tracks = lookup
                    .iter()
                    .map(|x| mgmt.database.tracks[x].clone())
                    .collect();
                let id = lookup[track[artist]];
                mgmt.load(id);
                mgmt.playlist.set(track[artist], tracks, lookup);
                self.play_ptr = self.ptr;
                mgmt.play();
            }
            State::Files {
                selection,
                list,
                dir,
            } => {
                let mut mgmt = self.mgmt.lock().unwrap();
                let selection = *selection;
                let file = &list[selection];
                let dir = dir.to_owned() + "/" + file;
                if is_file(&dir) {
                    mgmt.add_library(Track::load(&dir));
                    // Trigger library tree rerender
                    self.library_updated = true;
                } else {
                    let list = list_dir(&dir, !mgmt.config.show_hidden_files);
                    *self.states.get_mut(&self.ptr).unwrap() = State::Files {
                        selection: 0,
                        list,
                        dir,
                    };
                }
            }
            State::Playlists {
                playlist, track, ..
            } => {
                let mut mgmt = self.mgmt.lock().unwrap();
                mgmt.playlist.name = Some(playlist.to_string());
                if playlist.is_empty() {
                    return;
                }
                let display = mgmt.database.playlists[playlist].clone();
                if !display.is_empty() {
                    let tracks = display
                        .iter()
                        .map(|x| mgmt.database.tracks[x].clone())
                        .collect();
                    let id = display[track[playlist]];
                    mgmt.load(id);
                    mgmt.playlist.set(track[playlist], tracks, display);
                    self.play_ptr = self.ptr;
                    mgmt.play();
                }
            }
            _ => (),
        }
    }

    fn track_up(&mut self) {
        // Move track upwards
        let mut mgmt = self.mgmt.lock().unwrap();
        // Ensure there are available tracks
        if mgmt.database.tracks.is_empty() {
            return;
        }
        match self.state() {
            State::Library { selection, .. } => {
                if *selection != 0 {
                    // Update database
                    mgmt.database
                        .display
                        .simple
                        .swap(*selection, selection.saturating_sub(1));
                }
            }
            State::Playlists {
                depth,
                playlist,
                track,
                ..
            } => {
                if playlist.is_empty() {
                    return;
                }
                if *depth == 1 {
                    // Moving track display order around
                    let selection = track[playlist];
                    if selection != 0 {
                        mgmt.database
                            .playlists
                            .get_mut(playlist)
                            .unwrap()
                            .swap(selection, selection.saturating_sub(1));
                        std::mem::drop(mgmt);
                        self.selection_up();
                    }
                } else if *depth == 0 {
                    // Moving playlist display order around
                    let idx = mgmt
                        .database
                        .display
                        .playlists
                        .iter()
                        .position(|x| x == playlist);
                    if let Some(idx) = idx {
                        mgmt.database
                            .display
                            .playlists
                            .swap(idx, idx.saturating_sub(1));
                    }
                }
            }
            _ => (),
        }
    }

    fn track_down(&mut self) {
        // Move track downwards
        let mut mgmt = self.mgmt.lock().unwrap();
        // Ensure there are available tracks
        if mgmt.database.tracks.is_empty() {
            return;
        }
        match self.state() {
            State::Library { selection, .. } => {
                if *selection < mgmt.database.tracks.len().saturating_sub(1) {
                    // Update database
                    mgmt.database
                        .display
                        .simple
                        .swap(*selection, *selection + 1);
                }
            }
            State::Playlists {
                depth,
                playlist,
                track,
                ..
            } => {
                if playlist.is_empty() {
                    return;
                }
                if *depth == 1 {
                    // Move track display order around
                    let selection = track[playlist];
                    if selection < mgmt.database.playlists[playlist].len().saturating_sub(1) {
                        mgmt.database
                            .playlists
                            .get_mut(playlist)
                            .unwrap()
                            .swap(selection, selection + 1);
                        std::mem::drop(mgmt);
                        self.selection_down();
                    }
                } else if *depth == 0 {
                    // Moving playlist display order around
                    let idx = mgmt
                        .database
                        .display
                        .playlists
                        .iter()
                        .position(|x| x == playlist);
                    if let Some(idx) = idx {
                        if idx < mgmt.database.display.playlists.len().saturating_sub(1) {
                            mgmt.database.display.playlists.swap(idx, idx + 1);
                        }
                    }
                }
            }
            _ => (),
        }
    }

    fn selection_up(&mut self) {
        // Move the current selection down
        let artist_list = if self.state().is_sorted_library() {
            let mgmt = self.mgmt.lock().unwrap();
            let artists: Vec<String> = mgmt.library_tree.keys().map(|x| x.to_string()).collect();
            Some(artists)
        } else {
            None
        };
        let playlist_display = if self.state().is_playlists() {
            let mgmt = self.mgmt.lock().unwrap();
            Some(mgmt.database.display.playlists.clone())
        } else {
            None
        };
        match self.state_mut() {
            State::Library { selection, .. } => {
                if *selection > 0 {
                    *selection -= 1
                }
            }
            State::Files { selection, .. } => {
                if *selection > 0 {
                    *selection -= 1
                }
            }
            State::SortedLibrary {
                artist,
                track,
                depth,
                ..
            } => {
                let artists_idx = artist_list
                    .as_ref()
                    .unwrap()
                    .iter()
                    .position(|x| x == artist)
                    .unwrap_or(0);
                if *depth == 0 && artists_idx > 0 {
                    *artist = artist_list.unwrap()[artists_idx - 1].to_string();
                } else if *depth == 1 && track[artist] > 0 {
                    *track.get_mut(artist).unwrap() -= 1;
                }
            }
            State::Playlists {
                playlist,
                track,
                depth,
                ..
            } => {
                if playlist.is_empty() {
                    return;
                }
                if *depth == 0 {
                    let playlist_display = playlist_display.unwrap();
                    let idx = playlist_display
                        .iter()
                        .position(|x| x == playlist)
                        .unwrap_or(0);
                    *playlist = playlist_display[idx.saturating_sub(1)].to_string();
                } else if *depth == 1 {
                    *track.get_mut(playlist).unwrap() = track[playlist].saturating_sub(1);
                }
            }
            _ => (),
        }
    }

    fn selection_down(&mut self) {
        // Move the current selection down
        let tracks_len = self.mgmt.lock().unwrap().database.tracks.len();
        let artists_len = self.mgmt.lock().unwrap().library_tree.len();
        // If in sorted library, get list of tracks and artists
        let (track_list, artist_list) = if let State::SortedLibrary { artist, .. } = self.state() {
            let mgmt = self.mgmt.lock().unwrap();
            let artists: Vec<String> = mgmt.library_tree.keys().map(|x| x.to_string()).collect();
            (
                Some(artist_tracks(&mgmt.library_tree, artist)),
                Some(artists),
            )
        } else {
            (None, None)
        };
        // If in playlists, get playlist display
        let playlist_data = if let State::Playlists { playlist, .. } = self.state() {
            if playlist.is_empty() {
                return;
            }
            let mgmt = self.mgmt.lock().unwrap();
            Some((
                mgmt.database.display.playlists.clone(),
                mgmt.database.playlists[playlist].len(),
            ))
        } else {
            None
        };
        // Perform selection move
        match self.state_mut() {
            State::Library { selection, .. } => {
                if *selection + 1 < tracks_len {
                    *selection += 1
                }
            }
            State::Files {
                selection, list, ..
            } => {
                if *selection + 1 < list.len() {
                    *selection += 1
                }
            }
            State::SortedLibrary {
                artist,
                track,
                depth,
                ..
            } => {
                let artists_idx = artist_list
                    .as_ref()
                    .unwrap()
                    .iter()
                    .position(|x| x == artist)
                    .unwrap_or(0);
                if *depth == 0 && artists_idx + 1 < artists_len {
                    *artist = artist_list.unwrap()[artists_idx + 1].to_string();
                } else if *depth == 1 && track[artist] + 1 < track_list.unwrap().len() {
                    *track.get_mut(artist).unwrap() += 1;
                }
            }
            State::Playlists {
                playlist,
                track,
                depth,
                ..
            } => {
                let (playlist_display, tracks) = playlist_data.unwrap();
                if *depth == 0 {
                    let idx = playlist_display
                        .iter()
                        .position(|x| x == playlist)
                        .unwrap_or_else(|| playlist_display.len().saturating_sub(1));
                    if let Some(next) = playlist_display.get(idx + 1) {
                        *playlist = next.to_string();
                    }
                } else if *depth == 1 && track[playlist] + 1 < tracks {
                    *track.get_mut(playlist).unwrap() = track[playlist] + 1;
                }
            }
            _ => (),
        }
    }

    fn selection_top(&mut self) {
        // Move the selection to the top of the library
        let first_artist: Option<String> = if self.state().is_sorted_library() {
            let mgmt = self.mgmt.lock().unwrap();
            Some(
                mgmt.library_tree
                    .keys()
                    .nth(0)
                    .and_then(|x| Some(x.to_string()))
                    .unwrap_or_else(|| "".to_string()),
            )
        } else {
            None
        };
        // If in playlists, get playlist display
        let playlist_display = if self.state().is_playlists() {
            let mgmt = self.mgmt.lock().unwrap();
            Some(mgmt.database.display.playlists.clone())
        } else {
            None
        };
        match self.state_mut() {
            State::Library { selection } => {
                *selection = 0;
            }
            State::Files { selection, .. } => {
                *selection = 0;
            }
            State::SortedLibrary {
                depth,
                artist,
                track,
                ..
            } => {
                if *depth == 0 {
                    *artist = first_artist.unwrap();
                } else {
                    *track.get_mut(artist).unwrap() = 0;
                }
            }
            State::Playlists {
                depth,
                playlist,
                track,
                ..
            } => {
                if playlist.is_empty() {
                    return;
                }
                let playlist_display = playlist_display.unwrap();
                if *depth == 0 {
                    *playlist = playlist_display
                        .get(0)
                        .and_then(|x| Some(x.to_string()))
                        .unwrap_or_else(|| "".to_string());
                } else {
                    *track.get_mut(playlist).unwrap() = 0;
                }
            }
            _ => (),
        }
    }

    fn selection_bottom(&mut self) {
        // Move the selection to the top of the library
        let tracks_len = self.mgmt.lock().unwrap().database.tracks.len();
        // If in sorted library, get list of tracks in artist
        let (track_list, artist_list) = if let State::SortedLibrary { artist, .. } = self.state() {
            let mgmt = self.mgmt.lock().unwrap();
            let artists: Vec<String> = mgmt.library_tree.keys().map(|x| x.to_string()).collect();
            (
                Some(artist_tracks(&mgmt.library_tree, artist)),
                Some(artists),
            )
        } else {
            (None, None)
        };
        // If in playlists, get playlist display
        let playlist_data = if let State::Playlists { playlist, .. } = self.state() {
            if playlist.is_empty() {
                return;
            }
            let mgmt = self.mgmt.lock().unwrap();
            Some((
                mgmt.database.display.playlists.clone(),
                mgmt.database.playlists[playlist].len(),
            ))
        } else {
            None
        };
        match self.state_mut() {
            State::Library { selection } => {
                *selection = tracks_len.saturating_sub(1);
            }
            State::Files {
                selection, list, ..
            } => {
                *selection = list.len().saturating_sub(1);
            }
            State::SortedLibrary {
                depth,
                artist,
                track,
                ..
            } => {
                if *depth == 0 {
                    let artists_len = artist_list.as_ref().unwrap().len();
                    *artist =
                        artist_list.as_ref().unwrap()[artists_len.saturating_sub(1)].to_string();
                } else {
                    *track.get_mut(artist).unwrap() = track_list.unwrap().len().saturating_sub(1);
                }
            }
            State::Playlists {
                depth,
                playlist,
                track,
            } => {
                let (playlist_display, tracks) = playlist_data.unwrap();
                if *depth == 0 {
                    *playlist = playlist_display
                        .iter()
                        .last()
                        .and_then(|x| Some(x.to_string()))
                        .unwrap_or_else(|| "".to_string());
                } else {
                    *track.get_mut(playlist).unwrap() = tracks.saturating_sub(1);
                }
            }
            _ => (),
        }
    }

    pub fn update_library(&mut self) {
        // Prevent rendering with outdated library tree
        if self.library_updated && self.state().is_sorted_library() {
            let mut mgmt = self.mgmt.lock().unwrap();
            let tracks = &mgmt.database.tracks;
            mgmt.library_tree = form_library_tree(tracks);
            let artists: Vec<String> = mgmt.library_tree.keys().map(|x| x.to_string()).collect();
            std::mem::drop(mgmt);
            if let State::SortedLibrary {
                track,
                artist: artist_ptr,
                ..
            } = self.state_mut()
            {
                for artist in &artists {
                    if !track.contains_key(artist) {
                        track.insert(artist.to_string(), 0);
                    }
                }
                track.drain_filter(|t, _| !artists.contains(t));
                if !artists.contains(&artist_ptr) {
                    *artist_ptr = artists
                        .get(0)
                        .and_then(|x| Some(x.to_string()))
                        .unwrap_or_else(|| "".to_string());
                }
            }
            self.library_updated = false;
        }
    }

    pub fn render(&mut self) -> Result<()> {
        self.update_library();
        // Acquire manager
        let mgmt = self.mgmt.lock().unwrap();
        // Update library tree if need be
        // Obtain render data for the current state
        let ((keys, tracks), paths, artist_track, playlists): (
            TrackList,
            FileList,
            SortedList,
            OptionList,
        ) = match self.state() {
            State::Library { .. } => {
                // Obtain list of tracks
                let keys = mgmt.database.display.simple.clone();
                let tracks: Vec<&Track> = keys.iter().map(|x| &mgmt.database.tracks[x]).collect();
                let table = pad_table(format_table(&tracks), self.size.width as usize);
                ((Some(keys), Some(table)), None, None, None)
            }
            State::SortedLibrary {
                artist,
                track,
                depth,
                ..
            } => {
                let id_playing = if mgmt.playlist.is_ready() {
                    mgmt.playlist.current_id()
                } else {
                    None
                };
                let table = format_artist_track(
                    &mgmt.library_tree,
                    (artist.to_string(), track),
                    *depth,
                    &mgmt.database.tracks,
                    id_playing,
                    self.ptr == self.play_ptr,
                );
                ((None, None), None, Some(table), None)
            }
            State::Files { dir, .. } => {
                // Obtain list of files
                let files = list_dir(dir, !mgmt.config.show_hidden_files);
                ((None, None), Some(files), None, None)
            }
            State::Playlists {
                playlist,
                track,
                depth,
                ..
            } => {
                let playlists = format_playlist(
                    &mgmt.database.playlists,
                    &mgmt.database.display.playlists,
                    *depth,
                    &mgmt.database.tracks,
                    (&playlist, track),
                    mgmt.playlist.ptr,
                    &mgmt.playlist.name,
                    self.size.width,
                    &mgmt.config.indicators["playlist_icon"],
                );
                ((None, None), None, None, Some(playlists))
            }
            State::Empty => ((None, None), None, None, None),
        };
        std::mem::drop(mgmt);
        // Do render
        for line in 0..self.size.height {
            // Go to line and clear it
            queue!(
                self.stdout,
                cursor::MoveTo(0, line),
                terminal::Clear(ClearType::CurrentLine)
            )?;
            // Do maths
            let status_idx = self.size.height.saturating_sub(1);
            // Determine what to render on this line
            if line != status_idx && self.state().is_library() {
                queue!(self.stdout, terminal::Clear(ClearType::CurrentLine))?;
                // Acquire manager
                let mgmt = self.mgmt.lock().unwrap();
                // Render library view
                let selection = self.state().get_selection();
                if let Some(row) = tracks.as_ref().unwrap().get(line as usize) {
                    let is_selected = selection == line.into();
                    let this_id = keys
                        .as_ref()
                        .unwrap()
                        .get(line as usize)
                        .and_then(|i| Some(*i));
                    let is_playing = mgmt.playlist.is_ready()
                        && self.ptr == self.play_ptr
                        && mgmt.playlist.current_id() == this_id;
                    // Set up formatting for list
                    if is_selected {
                        queue!(self.stdout, SetBg(Color::DarkGrey))?;
                    }
                    if is_playing {
                        queue!(self.stdout, SetFg(Color::Green))?;
                    }
                    // Print row content
                    queue!(self.stdout, Print(row))?;
                    // Reset formatting for next row
                    queue!(self.stdout, SetBg(Color::Reset), SetFg(Color::Reset))?;
                } else if line == 0 {
                    // Print out placeholder
                    queue!(self.stdout, Print("[empty library]"))?;
                }
            } else if line != status_idx && self.state().is_files() {
                let selection = self.state().get_selection();
                if let Some(row) = paths.as_ref().unwrap().get(line as usize) {
                    // Add padding
                    let row = format!("{:<pad$}", row, pad = self.size.width as usize);
                    // Set up formatting for list
                    if selection == line.into() {
                        queue!(self.stdout, SetBg(Color::DarkGrey))?;
                    }
                    queue!(self.stdout, Print(row))?;
                    // Reset formatting for next row
                    queue!(self.stdout, SetBg(Color::Reset))?;
                }
            } else if line != status_idx && self.state().is_sorted_library() {
                if let Some(row) = artist_track.as_ref().unwrap().get(line as usize) {
                    // Add padding
                    let row = format!("{:<pad$}", row, pad = self.size.width as usize);
                    queue!(self.stdout, Print(row))?;
                    queue!(self.stdout, SetBg(Color::Reset), SetFg(Color::Reset))?;
                }
            } else if line != status_idx && self.state().is_playlists() {
                if let Some(row) = playlists.as_ref().unwrap().get(line as usize) {
                    queue!(self.stdout, Print(row))?;
                }
            } else if line == status_idx {
                // Render status line
                self.rerender_status()?;
            }
        }
        self.stdout.flush()
    }

    fn rerender_status(&mut self) -> Result<()> {
        // Render status line
        let mgmt = self.mgmt.lock().unwrap();
        // Form left hand side
        let lhs = if let Some(current) = mgmt.playlist.current() {
            let pb = mgmt.metadata.lock().unwrap().playback_status;
            let icon = match pb {
                PlaybackStatus::Playing => &mgmt.config.indicators["playing"],
                PlaybackStatus::Paused => &mgmt.config.indicators["paused"],
                PlaybackStatus::Stopped => &mgmt.config.indicators["stopped"],
            };
            format!("{}{} - {}", icon, current.tag.title, current.tag.artist)
        } else {
            "No track loaded".to_string()
        };
        // Obtain correct icons for current player state
        let md = mgmt.metadata.lock().unwrap();
        let loop_icon = match md.loop_status {
            LoopStatus::None => &mgmt.config.indicators["loop_none"],
            LoopStatus::Track => &mgmt.config.indicators["loop_track"],
            LoopStatus::Playlist => &mgmt.config.indicators["loop_playlist"],
        };
        let shuffle_icon = &mgmt.config.indicators[if md.shuffle_status {
            "shuffle_on"
        } else {
            "shuffle_off"
        }];
        #[allow(clippy::cast_possible_truncation)]
        let volume_icon = match (mgmt.player.volume() * 100.0) as u8 {
            // 0%: Mute icon
            0 => &mgmt.config.indicators["volume_mute"],
            // < 30%: Low speaker icon
            1..=30 => &mgmt.config.indicators["volume_low"],
            // < 60%: Medium speaker icon
            31..=60 => &mgmt.config.indicators["volume_medium"],
            // < 100%: Full speaker icon
            _ => &mgmt.config.indicators["volume_high"],
        };
        // Form right hand side
        #[allow(clippy::cast_possible_truncation)]
        let volume = (md.volume * 100.0) as usize;
        std::mem::drop(md);
        let (position, duration, percent) = if let Some(data) = mgmt.get_position() {
            data
        } else {
            mgmt.metadata.lock().unwrap().position
        };
        let rhs = format!(
            "{}/{} {}% {} {} {}",
            timefmt(position),
            timefmt(duration),
            volume,
            volume_icon,
            loop_icon,
            shuffle_icon
        );
        // Do alignment
        let space = align_sides(&lhs, &rhs, self.size.width as usize, 4).saturating_sub(4);
        if space > 3 {
            // Form progress bar
            #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
            let hl = ((space as f64 * percent) as usize).saturating_sub(1);
            let nohl = space - hl;
            let progress = format!(
                "|{}{}|",
                &mgmt.config.indicators["progress_bar_full"].repeat(hl),
                &mgmt.config.indicators["progress_bar_empty"].repeat(nohl)
            );
            // Put it all together and print it
            let status = format!("{} {} {}", lhs, progress, rhs);
            queue!(
                self.stdout,
                SetFg(Color::DarkBlue),
                Print(status),
                SetFg(Color::Reset)
            )?;
        }
        Ok(())
    }

    pub fn clean(&mut self) -> Result<()> {
        // Clean up before leaving
        self.mgmt.lock().unwrap().database.write();
        execute!(self.stdout, terminal::LeaveAlternateScreen, cursor::Show)?;
        terminal::disable_raw_mode()?;
        Ok(())
    }
}
