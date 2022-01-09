/*
    Synchron - A terminal music player
    - Allows control through dbus, integrating into your bar and playerctl
    - Reads ID3 tags from music
    - Can be controlled through prompt
    - Can play most mainstream formats
*/

#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::cast_sign_loss)]

#[macro_use]
mod util;
mod audio;
mod config;
mod mpris;
mod playlist;

use audio::{LoopStatus, Manager, PlaybackStatus, Track};
use mpris::Event;
use scanln::scanln;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn main() {
    // Build and initialise a manager
    let mut m = Manager::new();
    m.init();
    // Allow for it to be accessed from threads
    let m = Arc::new(Mutex::new(m));
    // Spawn a manager event loop, which handles mpris requests
    std::thread::spawn({
        let m = m.clone();
        move || {
            // Handle events
            loop {
                // Handle mpris event
                let mut m = m.lock().unwrap();
                if let Ok(e) = m.mpris.try_recv() {
                    match e {
                        Event::OpenUri(uri) => m.load(Track::load(&uri)),
                        Event::Pause => m.pause(),
                        Event::Play => m.play(),
                        Event::PlayPause => m.play_pause(),
                        Event::SetVolume(v) => m.set_volume(v),
                        Event::SetLoopStatus(s) => m.set_loop(s),
                        Event::SetShuffleStatus(s) => m.set_shuffle(s),
                        Event::SetPosition(p) => m.set_position(p),
                        Event::Seek(f, s) => m.seek(f, s),
                        Event::Stop => m.stop(),
                        Event::Next => println!("Next!"),
                        Event::Previous => println!("Previous!"),
                        Event::Raise | Event::Quit => (),
                    }
                }

                // Stop status after track has finished
                #[allow(clippy::float_cmp)]
                if m.get_position().2 == 1. {
                    m.metadata.lock().unwrap().playback_status = PlaybackStatus::Stopped;
                    m.next();
                }
                std::mem::drop(m);
                // Wait before next loop
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    });
    // Initiate a control prompt for the player
    loop {
        let cmd = scanln!("{}", m.lock().unwrap().config.prompt);
        let mut m = m.lock().unwrap();
        match cmd.as_str().split(' ').collect::<Vec<&str>>().as_slice() {
            // Opening media
            ["open", o @ ..] => m.load(Track::load(&o.join(" "))),
            // Playlist handling
            ["queue", o @ ..] => m.queue(Track::load(&o.join(" "))),
            ["next"] => m.next().unwrap_or(()),
            ["prev"] => m.previous().unwrap_or(()),
            // Metadata
            ["status"] => {
                let (p, d, pr) = m.get_position();
                println!("{}s / {}s ({:.2}%)\n", p, d, pr * 100.);
                print!("{}", m.playlist.view());
            }
            // Playing and pausing commands
            ["toggle"] => m.play_pause(),
            ["play"] => m.play(),
            ["pause"] => m.pause(),
            ["stop"] => m.stop(),
            // Loop controls
            ["loop", "off"] => m.set_loop(LoopStatus::None),
            ["loop", "track"] => m.set_loop(LoopStatus::Track),
            ["loop", "playlist"] => m.set_loop(LoopStatus::Playlist),
            ["loop", "get"] => println!("{:?}", m.metadata.lock().unwrap().loop_status),
            // Shuffle controls
            ["shuffle", "on"] => m.set_shuffle(true),
            ["shuffle", "off"] => m.set_shuffle(false),
            ["shuffle", "get"] => println!(
                "{}",
                if m.metadata.lock().unwrap().shuffle_status {
                    "On"
                } else {
                    "Off"
                }
            ),
            // Volume controls
            ["volume", "up"] => {
                let volume = m.metadata.lock().unwrap().volume;
                m.set_volume(volume + 0.3);
            }
            ["volume", "down"] => {
                let volume = m.metadata.lock().unwrap().volume;
                m.set_volume(volume - 0.3);
            }
            ["volume", "set", v] => m.set_volume(v.parse().unwrap_or(1.0)),
            ["volume", "get"] => println!("{}", m.metadata.lock().unwrap().volume),
            ["volume", "reset"] => m.set_volume(1.0),
            // Position controls
            ["position", "set", p] => m.set_position(p.parse().unwrap_or(-1)),
            ["position", "get"] => {
                let (p, d, pr) = m.get_position();
                println!("{}s / {}s ({:.2}%)", p, d, pr * 100.);
            }
            ["seek", "backward"] => m.seek(false, Duration::from_secs(5)),
            ["seek", "forward"] => m.seek(true, Duration::from_secs(5)),
            // Exit player
            ["exit"] => std::process::exit(0),
            // Unknown command
            _ => println!("Unknown command: '{}'", cmd),
        }
        std::mem::drop(m);
    }
}
