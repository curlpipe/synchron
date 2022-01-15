// ui.rs - controls and renders the TUI
use crate::audio::{LoopStatus, Manager, PlaybackStatus};
use crate::config::PULSE;
use crate::track::Track;
use crate::util::{align_sides, format_table, pad_table, track_list_display, timefmt};
pub use crossterm::{
    cursor,
    event::{self, Event, KeyCode as KCode, KeyEvent, KeyModifiers as KMod},
    execute, queue,
    style::{self, Print, SetForegroundColor as SetFg, SetBackgroundColor as SetBg, Color},
    terminal::{self, ClearType},
    Command, Result,
};
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;

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

pub enum State {
    Library { selection: usize },
    Empty,
}

impl State {
    pub fn is_library(&self) -> bool {
        match self {
            Self::Library { .. } => true,
            _ => false,
        }
    }
}

pub struct Ui {
    stdout: std::io::Stdout,
    mgmt: Arc<Mutex<Manager>>,
    state: State,
    size: Size,
    active: bool,
}

impl Ui {
    pub fn new(m: Arc<Mutex<Manager>>) -> Result<Self> {
        Ok(Self {
            stdout: std::io::stdout(),
            mgmt: m,
            state: State::Library { selection: 0 },
            size: Size::screen()?,
            active: true,
        })
    }

    pub fn init(&mut self) -> Result<()> {
        // Initiate the UI
        execute!(self.stdout, terminal::EnterAlternateScreen, cursor::Hide)?;
        terminal::enable_raw_mode()?;
        // Handle any panics that may occur
        std::panic::set_hook(Box::new(|e| {
            terminal::disable_raw_mode().unwrap();
            execute!(
                std::io::stdout(),
                terminal::LeaveAlternateScreen,
                cursor::Show
            )
            .unwrap();
            eprintln!("{}", e);
        }));
        Ok(())
    }

    pub fn run(&mut self) -> Result<()> {
        // Run the UI
        self.render()?;
        while self.active {
            if event::poll(std::time::Duration::from_millis(PULSE))? {
                match event::read()? {
                    Event::Key(k) => self.on_key(k)?,
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
            if self.mgmt.lock().unwrap().metadata.lock().unwrap().playback_status == PlaybackStatus::Playing {
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

    pub fn on_key(&mut self, e: KeyEvent) -> Result<()> {
        // Handle key event
        match (e.modifiers, e.code) {
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
            // [Enter] : Play selection
            (KMod::NONE, KCode::Enter) => self.select(),
            // [/\] : Move up selection in library
            (KMod::NONE, KCode::Up) => self.selection_up(),
            // [\/] : Move down selection in library
            (KMod::NONE, KCode::Down) => self.selection_down(),
            // [Ctrl] + [\/] : Move selection to top of library
            (KMod::CONTROL, KCode::Up) => self.selection_top(),
            // [Ctrl] + [/\] : Move selection to bottom of library
            (KMod::CONTROL, KCode::Down) => self.selection_bottom(),
            // [<] : Seek backward 5 seconds
            (KMod::NONE, KCode::Left) => self.mgmt.lock().unwrap().seek(false, Duration::from_secs(5)),
            // [>] : Seek forward 5 seconds
            (KMod::NONE, KCode::Right) => self.mgmt.lock().unwrap().seek(true, Duration::from_secs(5)),
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
                let v = self.mgmt.lock().unwrap().metadata.lock().unwrap().volume;
                self.mgmt.lock().unwrap().set_volume(v + 0.1);
            }
            // [Shift] + [\/] : Volume down
            (KMod::SHIFT, KCode::Down) => {
                let v = self.mgmt.lock().unwrap().metadata.lock().unwrap().volume;
                self.mgmt.lock().unwrap().set_volume(v - 0.1);
            }
            // [;] or [:] : Command mode
            (KMod::NONE, KCode::Char(':' | ';')) => (),
            // [???] : Do nothing
            _ => (),
        }
        Ok(())
    }

    fn select(&mut self) {
        // Play the selected track
        if let State::Library { selection } = self.state {
            let mut mgmt = self.mgmt.lock().unwrap();
            let lookup = track_list_display(&mgmt.database.tracks);
            let id = lookup[selection];
            mgmt.load(id);
            mgmt.play();
        }
    }

    fn selection_up(&mut self) {
        // Move the current selection down
        if let State::Library { selection } = &mut self.state {
            if *selection > 0 {
                *selection -= 1;
            }
        }
    }

    fn selection_down(&mut self) {
        // Move the current selection down
        if let State::Library { selection } = &mut self.state {
            if *selection + 1 < self.mgmt.lock().unwrap().database.tracks.len() {
                *selection += 1;
            }
        }
    }

    fn selection_top(&mut self) {
        // Move the selection to the top of the library
        if let State::Library { selection } = &mut self.state {
            *selection = 0;
        }
    }

    fn selection_bottom(&mut self) {
        // Move the selection to the top of the library
        if let State::Library { selection } = &mut self.state {
            *selection = track_list_display(&self.mgmt.lock().unwrap().database.tracks).len().saturating_sub(1);
        }
    }

    pub fn render(&mut self) -> Result<()> {
        // Acquire manager
        let mgmt = self.mgmt.lock().unwrap();
        // Get list of tracks
        // NOTE: Make sure to "if" this when more states are added for optimisation
        let keys = track_list_display(&mgmt.database.tracks);
        let tracks: Vec<&Track> = keys
            .iter()
            .map(|x| &mgmt.database.tracks[x])
            .collect();
        let table = pad_table(format_table(tracks), self.size.width as usize);
        std::mem::drop(mgmt);
        // Do render
        for line in 0..self.size.height {
            // Go to line and clear it
            queue!(self.stdout, cursor::MoveTo(0, line), terminal::Clear(ClearType::CurrentLine))?;
            // Do maths
            let status_idx = self.size.height.saturating_sub(1);
            // Determine what to render on this line
            if line != status_idx && self.state.is_library() {
                queue!(self.stdout, terminal::Clear(ClearType::CurrentLine))?;
                // Acquire manager
                let mgmt = self.mgmt.lock().unwrap();
                // Render library view
                if let State::Library { selection } = self.state {
                    if let Some(row) = table.get(line as usize) {
                        let is_selected = selection == line.into();
                        let is_playing = mgmt.playlist.current_id() == Some(keys[line as usize]);
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
                    }
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
            let icon = mgmt.metadata.lock().unwrap().playback_status;
            let icon = icon.icon();
            format!("{}{} - {}", icon, current.tag.title, current.tag.artist)
        } else {
            "No track loaded".to_string()
        };
        // Obtain correct icons for current player state
        let md = mgmt.metadata.lock().unwrap();
        let loop_icon = match md.loop_status {
            LoopStatus::None => "稜",
            LoopStatus::Track => "綾",
            LoopStatus::Playlist => "凌",
        };
        let shuffle_icon = if md.shuffle_status { "列" } else { "劣" };
        let volume_icon = match (mgmt.player.volume() * 100.0) as u8 {
            // 0%: Mute icon
            0 => "ﱝ ",
            // < 30%: Low speaker icon
            0..=30 => "奄",
            // < 60%: Medium speaker icon
            31..=60 => "奔",
            // < 100%: Full speaker icon
            _ => "墳",
        };
        // Form right hand side
        let volume = (md.volume * 100.0) as usize;
        std::mem::drop(md);
        let (position, duration, percent) = if let Some(data) = mgmt.get_position() {
            data
        } else {
            mgmt.metadata.lock().unwrap().position
        };
        let rhs = format!("{}/{} {}% {} {} {}", timefmt(position), timefmt(duration), volume, volume_icon, loop_icon, shuffle_icon);
        // Do alignment
        let space = align_sides(&lhs, &rhs, self.size.width as usize, 4).saturating_sub(4);
        if space > 3 {
            // Form progress bar
            let hl = ((space as f64 * percent) as usize).saturating_sub(1);
            let nohl = space - hl;
            let progress = format!("|{}{}|", "".repeat(hl), " ".repeat(nohl));
            // Put it all together and print it
            let status = format!("{} {} {}", lhs, progress, rhs);
            queue!(self.stdout, SetFg(Color::DarkBlue), Print(status), SetFg(Color::Reset))?;
        }
        Ok(())
    }

    pub fn clean(&mut self) -> Result<()> {
        // Clean up before leaving
        execute!(self.stdout, terminal::LeaveAlternateScreen, cursor::Show)?;
        terminal::disable_raw_mode()?;
        Ok(())
    }
}
