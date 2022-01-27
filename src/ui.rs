// ui.rs - controls and renders the TUI
use crate::audio::{LoopStatus, Manager, PlaybackStatus};
use crate::config::{Pane, PULSE};
use crate::track::Track;
use crate::util::{
    align_sides, artist_tracks, expand_path, form_library_tree, format_artist_track, format_table,
    is_file, list_dir, pad_table, timefmt,
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
        artist: usize,
        album: usize,
        track: HashMap<usize, usize>,
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

    pub fn get_selection(&self) -> usize {
        match self {
            Self::Library { selection, .. } => *selection,
            Self::Files { selection, .. } => *selection,
            Self::SortedLibrary {
                artist,
                track,
                depth,
                ..
            } => {
                if *depth == 0 {
                    *artist
                } else {
                    track[artist]
                }
            }
            Self::Empty => unreachable!(),
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
        let artists = mgmt.library_tree.keys().len();
        let mut track = HashMap::new();
        for i in 0..artists {
            track.insert(i, 0);
        }
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
                        album: 0,
                        artist: 0,
                    },
                    Pane::Files => {
                        let dir = expand_path("~/").unwrap_or_else(|| ".".to_string());
                        State::Files {
                            selection: 0,
                            list: list_dir(&dir, !mgmt.config.show_hidden_files),
                            dir,
                        }
                    }
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
            }
            // Rerender the status line if playing, to keep up with the position of the song
            let status = get_md!(self.mgmt).playback_status;
            if status == PlaybackStatus::Playing {
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
            // [d] : Delete from library
            (KMod::NONE, KCode::Char('d')) => self.remove(),
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
            // [;] or [:] : Command mode
            (KMod::NONE, KCode::Char(':' | ';')) => (),
            // [???] : Do nothing
            _ => (),
        }
    }

    fn get_selected_id(&self) -> Option<usize> {
        // Get the track id that is selected (state independent)
        Some(match self.state() {
            State::Library { selection, .. } => {
                self.mgmt.lock().unwrap().database.display[*selection]
            }
            State::SortedLibrary {
                artist,
                track,
                depth: 1,
                ..
            } => artist_tracks(&self.mgmt.lock().unwrap().library_tree, *artist)[track[artist]],
            _ => return None,
        })
    }

    fn deepen(&mut self) {
        // Switch focus in the sorted library view
        if let State::SortedLibrary { depth, .. } = self.state_mut() {
            if depth == &1 {
                *depth = 0;
            } else {
                *depth += 1;
            }
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
            }
            // Rerender the status line if playing, to keep up with the position of the song
            let status = get_md!(self.mgmt).playback_status;
            if status == PlaybackStatus::Playing {
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
        // Update library tree if necessary
        if self.library_updated {
            let mut mgmt = self.mgmt.lock().unwrap();
            let tracks = &mgmt.database.tracks;
            mgmt.library_tree = form_library_tree(tracks);
            self.library_updated = false;
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
                let mut mgmt = self.mgmt.lock().unwrap();
                // Get track ID
                if let Some(id) = self.get_selected_id() {
                    mgmt.remove_library(id);
                    // Check for selection issues
                    if selection > &mgmt.database.display.len().saturating_sub(2) {
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
                    let tracks = artist_tracks(&mgmt.library_tree, *artist);
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
                // Ensure there are available tracks
                if mgmt.database.tracks.is_empty() {
                    return;
                }
                let lookup = mgmt.database.display.clone();
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
                let lookup = artist_tracks(&mgmt.library_tree, *artist);
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
        if let State::Library { selection, .. } = self.state() {
            if *selection != 0 {
                // Update database
                mgmt.database
                    .display
                    .swap(*selection, selection.saturating_sub(1));
            }
        }
        std::mem::drop(mgmt);
        self.selection_up();
    }

    fn track_down(&mut self) {
        // Move track downwards
        let mut mgmt = self.mgmt.lock().unwrap();
        // Ensure there are available tracks
        if mgmt.database.tracks.is_empty() {
            return;
        }
        if let State::Library { selection, .. } = self.state() {
            if *selection < mgmt.database.tracks.len().saturating_sub(1) {
                // Update database
                mgmt.database.display.swap(*selection, *selection + 1);
            }
        }
        std::mem::drop(mgmt);
        self.selection_down();
    }

    fn selection_up(&mut self) {
        // Move the current selection down
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
                if *depth == 0 && *artist > 0 {
                    *artist -= 1
                } else if *depth == 1 && track[artist] > 0 {
                    *track.get_mut(artist).unwrap() -= 1;
                }
            }
            _ => (),
        }
    }

    fn selection_down(&mut self) {
        // Move the current selection down
        let tracks = self.mgmt.lock().unwrap().database.tracks.len();
        let artists = self.mgmt.lock().unwrap().library_tree.len();
        // If in sorted library, get list of tracks in artist
        let track_list = if let State::SortedLibrary { artist, .. } = self.state() {
            let mgmt = self.mgmt.lock().unwrap();
            Some(artist_tracks(&mgmt.library_tree, *artist))
        } else {
            None
        };
        // Perform selection move
        match self.state_mut() {
            State::Library { selection, .. } => {
                if *selection + 1 < tracks {
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
                if *depth == 0 && *artist + 1 < artists {
                    *artist += 1
                } else if *depth == 1 && track[artist] + 1 < track_list.unwrap().len() {
                    *track.get_mut(artist).unwrap() += 1;
                }
            }
            _ => (),
        }
    }

    fn selection_top(&mut self) {
        // Move the selection to the top of the library
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
                    *artist = 0;
                } else {
                    *track.get_mut(artist).unwrap() = 0;
                }
            }
            _ => (),
        }
    }

    fn selection_bottom(&mut self) {
        // Move the selection to the top of the library
        let tracks = self.mgmt.lock().unwrap().database.tracks.len();
        let artists = self.mgmt.lock().unwrap().library_tree.len();
        // If in sorted library, get list of tracks in artist
        let track_list = if let State::SortedLibrary { artist, .. } = self.state() {
            let mgmt = self.mgmt.lock().unwrap();
            Some(artist_tracks(&mgmt.library_tree, *artist))
        } else {
            None
        };
        match self.state_mut() {
            State::Library { selection } => {
                *selection = tracks.saturating_sub(1);
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
                    *artist = artists.saturating_sub(1);
                } else {
                    *track.get_mut(artist).unwrap() = track_list.unwrap().len().saturating_sub(1);
                }
            }
            _ => (),
        }
    }

    pub fn render(&mut self) -> Result<()> {
        // Acquire manager
        let mut mgmt = self.mgmt.lock().unwrap();
        // Obtain render data for the current state
        let ((keys, tracks), paths, artist_track): (TrackList, FileList, SortedList) = match self
            .state()
        {
            State::Library { .. } => {
                // Obtain list of tracks
                let keys = mgmt.database.display.clone();
                let tracks: Vec<&Track> = keys.iter().map(|x| &mgmt.database.tracks[x]).collect();
                let table = pad_table(format_table(&tracks), self.size.width as usize);
                ((Some(keys), Some(table)), None, None)
            }
            State::SortedLibrary {
                artist,
                album,
                track,
                depth,
                ..
            } => {
                // Prevent rendering with outdated library tree
                if self.library_updated {
                    let tracks = &mgmt.database.tracks;
                    mgmt.library_tree = form_library_tree(tracks);
                }
                let id_playing = if mgmt.playlist.is_ready() {
                    mgmt.playlist.current_id()
                } else {
                    None
                };
                let table = format_artist_track(
                    &mgmt.library_tree,
                    (*artist, *album, track),
                    *depth,
                    &mgmt.database.tracks,
                    id_playing,
                );
                self.library_updated = false;
                ((None, None), None, Some(table))
            }
            State::Files { dir, .. } => {
                // Obtain list of files
                let files = list_dir(dir, !mgmt.config.show_hidden_files);
                ((None, None), Some(files), None)
            }
            State::Empty => ((None, None), None, None),
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
