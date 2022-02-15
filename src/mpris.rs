// mpris.rs - handling mpris interactions
use crate::audio::{LoopStatus, Metadata};
use crate::config::DBUS_PULSE;
use crate::track::Tag;
use dbus::arg::{RefArg, Variant};
use dbus::blocking::Connection;
use dbus::channel::MatchingReceiver;
use dbus::ffidisp::stdintf::org_freedesktop_dbus::PropertiesPropertiesChanged as Ppc;
use dbus::message::SignalArgs;
use dbus::strings::Path as DbusPath;
use dbus::MethodErr;
use dbus_crossroads::{Crossroads, IfaceBuilder};
use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

// Types
type EventHandler = Arc<Mutex<dyn Fn(Event) + Send + 'static>>;

// Representation of control events
#[derive(Clone, Debug)]
pub enum Event {
    OpenUri(String),
    SetLoopStatus(LoopStatus),
    SetShuffleStatus(bool),
    SetPosition(i64),
    SetVolume(f64),
    Seek(bool, Duration),
    Play,
    Pause,
    PlayPause,
    Next,
    Previous,
    Stop,
    Raise,
    Quit,
}

#[allow(clippy::too_many_lines)]
pub fn connect(ev: EventHandler, md: &Arc<Mutex<Metadata>>, update: &mpsc::Receiver<()>) {
    // Names of the player
    let name = "synchron".to_string();
    let name2 = "org.mpris.MediaPlayer2.synchron";
    // Establish connection to dbus
    let c = Connection::new_session().unwrap();
    c.request_name(name2, false, true, false).unwrap();
    let mut cr = Crossroads::new();
    // Register MediaPlayer2
    let mp2 = cr.register("org.mpris.MediaPlayer2", {
        let ev = ev.clone();
        move |b| {
            register(b, &ev, "Raise", Event::Raise);
            register(b, &ev, "Quit", Event::Quit);
            b.property("Identity").get(move |_, _| Ok(name.clone()));
            b.property("CanQuit").get(move |_, _| Ok(true));
            b.property("CanRaise").get(move |_, _| Ok(true));
            b.property("HasTrackList").get(move |_, _| Ok(false));
            b.property("SupportedUriSchemes")
                .get(move |_, _| Ok(&[] as &[String]));
            b.property("SupportedMimeTypes")
                .get(move |_, _| Ok(&[] as &[String]));
        }
    });
    // Register Player
    let player_md = md.clone();
    let mp2p = cr.register("org.mpris.MediaPlayer2.Player", move |b| {
        // Register play, pause, next, preivous and stop events
        register(b, &ev, "Play", Event::Play);
        register(b, &ev, "Pause", Event::Pause);
        register(b, &ev, "PlayPause", Event::PlayPause);
        register(b, &ev, "Next", Event::Next);
        register(b, &ev, "Previous", Event::Previous);
        register(b, &ev, "Stop", Event::Stop);
        // Necessary for mpris
        b.property("CanControl").get(|_, _| Ok(true));
        b.property("CanPlay").get(|_, _| Ok(true));
        b.property("CanPause").get(|_, _| Ok(true));
        b.property("CanGoNext").get(|_, _| Ok(true));
        b.property("CanGoPrevious").get(|_, _| Ok(true));
        b.property("CanSeek").get(|_, _| Ok(true));
        // Get the playback status from the metadata
        b.property("PlaybackStatus").get({
            let md = player_md.clone();
            move |_, _| Ok(format!("{:?}", md.lock().unwrap().playback_status))
        });
        // Get and set the loop status from the metadata
        b.property("LoopStatus")
            .get({
                let md = player_md.clone();
                move |_, _| Ok(format!("{:?}", md.lock().unwrap().loop_status))
            })
            .set({
                let ev = ev.clone();
                move |_, _, status| {
                    // Trigger loop set event
                    (ev.lock().unwrap())(Event::SetLoopStatus(match status.as_str() {
                        "Track" => LoopStatus::Track,
                        "Playlist" => LoopStatus::Playlist,
                        _ => LoopStatus::None,
                    }));
                    Ok(None)
                }
            });
        // Get and set the shuffle status from the metadata
        b.property("Shuffle")
            .get({
                let md = player_md.clone();
                move |_, _| Ok(md.lock().unwrap().shuffle_status)
            })
            .set({
                let ev = ev.clone();
                move |_, _, status| {
                    // Trigger shuffle set event
                    (ev.lock().unwrap())(Event::SetShuffleStatus(status));
                    Ok(None)
                }
            });
        // Get the position status from the metadata
        b.property("Position").get({
            let md = player_md.clone();
            move |_, _| -> Result<i64, MethodErr> {
                Ok(md.lock().unwrap().position.0.try_into().unwrap())
            }
        });
        b.property("Volume")
            .get({
                let md = player_md.clone();
                move |_, _| -> Result<f64, MethodErr> { Ok(md.lock().unwrap().volume) }
            })
            .set({
                let ev = ev.clone();
                move |_, _, volume| {
                    // Trigger volume set event
                    (ev.lock().unwrap())(Event::SetVolume(volume));
                    Ok(None)
                }
            });
        // Get and format the track information from the metadata
        b.property("Metadata").get({
            let md = player_md.clone();
            move |_, _| {
                let mut export = mpris_metadata(&md.lock().unwrap().tag);
                export.insert(
                    "mpris:trackid".to_string(),
                    Variant(Box::new(DbusPath::new("/").unwrap())),
                );
                Ok(export)
            }
        });
        // Method to set the position as requested through dbus
        b.method("SetPosition", ("TrackID", "Position"), (), {
            let ev = ev.clone();
            move |_, _, (_, position): (DbusPath, i64)| {
                // Send to event handler, in the correct format (seconds)
                (ev.lock().unwrap())(Event::SetPosition(
                    Duration::from_micros(position.try_into().unwrap_or(0))
                        .as_secs()
                        .try_into()
                        .unwrap_or(0),
                ));
                Ok(())
            }
        });
        // Method for seeking
        b.method("Seek", ("Offset",), (), {
            let ev = ev.clone();
            move |_, _, (offset,): (i64,)| {
                // Work out direction and magnitude of seek
                let magnitude = offset.abs() as u64;
                let forwards = offset > 0;
                // Send to event handler
                (ev.lock().unwrap())(Event::Seek(forwards, Duration::from_micros(magnitude)));
                Ok(())
            }
        });
        // Method to open a new media file
        b.method("OpenUri", ("Uri",), (), {
            move |_, _, (uri,): (String,)| {
                // Send to event handler
                (ev.lock().unwrap())(Event::OpenUri(uri));
                Ok(())
            }
        });
    });
    // Insert into mpris
    cr.insert("/org/mpris/MediaPlayer2", &[mp2, mp2p], ());
    // Start recieving events
    c.start_receive(
        dbus::message::MatchRule::new_method_call(),
        Box::new(move |msg, conn| {
            cr.handle_message(msg, conn).unwrap();
            true
        }),
    );
    // Start server loop
    loop {
        if update.try_recv().is_ok() {
            // When an update event is received, update information in the player
            let m = md.lock().unwrap();
            let mut changed = Ppc {
                interface_name: "org.mpris.MediaPlayer2.Player".to_string(),
                ..Ppc::default()
            };
            // Attach information
            add_prop!(
                changed.changed_properties,
                "PlaybackStatus",
                format!("{:?}", m.playback_status)
            );
            add_prop!(
                changed.changed_properties,
                "LoopStatus",
                format!("{:?}", m.loop_status)
            );
            add_prop!(changed.changed_properties, "Shuffle", m.shuffle_status);
            add_prop!(changed.changed_properties, "Volume", m.volume);
            add_prop!(
                changed.changed_properties,
                "Metadata",
                mpris_metadata(&m.tag)
            );
            // Send the message
            c.channel()
                .send(changed.to_emit_message(
                    &DbusPath::new("/org/mpris/MediaPlayer2".to_string()).unwrap(),
                ))
                .unwrap();
        }
        // Wait before checking again
        c.process(std::time::Duration::from_millis(DBUS_PULSE))
            .unwrap();
    }
}

pub fn register(b: &mut IfaceBuilder<()>, ev: &EventHandler, name: &'static str, event: Event) {
    // Register a new event for an event handler
    let ev = ev.clone();
    b.method(name, (), (), move |_, _, _: ()| {
        (ev.lock().unwrap())(event.clone());
        Ok(())
    });
}

fn mpris_metadata(tag: &Tag) -> HashMap<String, Variant<Box<dyn RefArg>>> {
    // Create a hashmap of id3 tags for mpris
    let mut md: HashMap<String, Variant<Box<dyn RefArg>>> = HashMap::new();
    add_prop!(md, "xesam:title", tag.title.clone());
    add_prop!(md, "xesam:album", tag.album.clone());
    add_prop!(md, "xesam:artist", tag.artist.clone());
    add_prop!(md, "xesam:year", tag.year.clone());
    md
}
