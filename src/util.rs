// util.rs - common utilities for helping out around the project
use crate::track::Track;
use crate::ui::{Color, SetBg, SetFg};
use crossterm::style::{Attribute as Attr, SetAttribute as SetAttr};
use std::collections::HashMap;
use unicode_width::UnicodeWidthStr;

// Help text
pub const HELP: &str = "Synchron:
    About:
        Synchron is a music player that can be run as a TUI or as a CLI. 
        It provides a way to organise, download, play and tag your
        music, podcasts, audiobooks and other forms of media.
        Please refer to the guide at https://github.com/curlpipe/synchron
        to get started.
    Options:
        -h, --help    : Prints this help message.
        -V, --version : Prints the version installed.
        -c, --cli     : Enters into CLI mode which displays a prompt that waits
                        for commands to be entered.
    Examples:
        synchron -h   : Show help message and exit.
        synchron -V   : Show version and exit.
        synchron      : Opens in the default TUI mode.
        synchron -c   : Opens in CLI mode and awaits for your instructions.";

// Utility macro for easy dbus property addition
#[macro_export]
macro_rules! add_prop {
    ($props:expr, $prop:expr, $value:expr) => {
        $props.insert($prop.to_string(), Variant(Box::new($value)));
    };
}

// Utility macro for getting metadata from manager
#[macro_export]
macro_rules! get_md {
    ($mgmt:expr) => {
        $mgmt.lock().unwrap().metadata.lock().unwrap()
    };
}

pub fn expand_path(path: &str) -> Option<String> {
    // Utility function for expanding paths
    let with_user = expanduser::expanduser(path).ok()?;
    let full_path = std::fs::canonicalize(with_user).ok()?;
    full_path.into_os_string().into_string().ok()
}

pub fn attempt_open(path: &str) -> Option<String> {
    // Attempt to open a file from an unstandardised path
    let path = expand_path(path)?;
    std::fs::read_to_string(path).ok()
}

pub fn width(s: &str, tab: usize) -> usize {
    // Find width of a string
    let s = s.replace('\t', &" ".repeat(tab));
    s.width()
}

pub fn pad_table(table: Vec<Vec<String>>, limit: usize) -> Vec<String> {
    // Check table isn't empty
    if table.is_empty() {
        return vec![];
    }
    // Apply padding to table and form into strings
    let mut result = vec![];
    // Calculate the lengths needed
    let length: usize = table[0].iter().map(|x| x.width()).sum();
    let inner = table[0].len().saturating_sub(1);
    // Determine if columns will be able to fit
    if length + inner < limit {
        // Columns will fit, distribute spacing between them
        let total = limit - length;
        let gaps = if inner == 0 {
            [0, 0, 0]
        } else {
            let gap = total / inner;
            let mut left_over = total % inner;
            let mut gaps = [gap, gap, gap];
            for i in gaps.iter_mut().take(2) {
                if left_over != 0 {
                    *i += 1;
                    left_over -= 1;
                }
            }
            gaps
        };
        // Format columns into strings
        for record in table {
            let mut row = String::new();
            for i in 0..4 {
                if record.len() > i {
                    row.push_str(&record[i]);
                    if record.len() > i + 1 {
                        row.push_str(&" ".repeat(gaps[i]));
                    }
                }
            }
            if record.len() > 4 {
                row.push_str(&record[4]);
            }
            result.push(row);
        }
    } else {
        // Recalculate padding with new column amount (rely on recursion)
        result = match table[0].len() {
            4 | 2 => pad_table(remove_column(table, 1), limit),
            3 => pad_table(remove_column(table, 2), limit),
            1 => (0..table.len()).map(|_| "...".to_string()).collect(),
            _ => vec![],
        }
    }
    result
}

pub fn remove_column(mut table: Vec<Vec<String>>, column: usize) -> Vec<Vec<String>> {
    // Remove a column from a table
    for i in &mut table {
        i.remove(column);
    }
    table
}

pub fn format_table(tracks: &[&Track]) -> Vec<Vec<String>> {
    // Format a list of tracks into a table
    let mut result = vec![];
    let tracks: Vec<(String, &String, &String, &String, &String)> =
        tracks.iter().map(|x| x.format_elements()).collect();
    // Sort into columns
    let columns: Vec<Vec<&String>> = vec![
        tracks.iter().map(|x| x.1).collect(),
        tracks.iter().map(|x| x.2).collect(),
        tracks.iter().map(|x| x.3).collect(),
        tracks.iter().map(|x| x.4).collect(),
    ];
    // Find the longest item in each column
    let mut limits = vec![];
    for column in &columns {
        limits.push(find_longest(column));
    }
    // Reform back into rows, taking into account the maximum column size
    for i in 0..tracks.len() {
        let mut row = vec![];
        row.push(align_left(columns[0][i], limits[0]));
        row.push(align_left(columns[1][i], limits[1]));
        row.push(align_left(columns[2][i], limits[2]));
        row.push(align_left(columns[3][i], limits[3]));
        result.push(row);
    }
    result
}

pub fn find_longest(target: &[&String]) -> usize {
    // Find the longest string in a vector
    let mut longest = 0;
    for i in target {
        if i.width() > longest {
            longest = i.width();
        }
    }
    longest
}

pub fn align_left(target: &str, space: usize) -> String {
    let pad = " ".repeat(space.saturating_sub(target.width()));
    format!("{}{}", target, pad)
}

pub fn align_sides(lhs: &str, rhs: &str, space: usize, tab_width: usize) -> usize {
    // Align left and right hand side
    let total = width(lhs, tab_width) + width(rhs, tab_width);
    if total > space {
        0
    } else {
        space.saturating_sub(total)
    }
}

pub fn timefmt(duration: u64) -> String {
    let minutes: u64 = duration / 60;
    let seconds: u64 = duration % 60;
    format!("{}:{:02}", minutes, seconds)
}

pub fn is_file(path: &str) -> bool {
    std::path::Path::new(path).is_file()
}

pub fn list_dir(path: &str, no_hidden: bool) -> Vec<String> {
    let mut files: Vec<String> = std::fs::read_dir(path)
        .unwrap()
        .map(|d| d.unwrap().file_name().into_string().unwrap())
        .filter(|d| if no_hidden { !d.starts_with(".") } else { true })
        .collect();
    files.push("..".to_string());
    files.sort();
    files
}

pub fn form_library_tree(
    tracks: &HashMap<usize, Track>,
) -> HashMap<String, HashMap<String, Vec<usize>>> {
    // Create a library tree from a list of tracks
    let mut result: HashMap<String, HashMap<String, Vec<usize>>> = HashMap::new();
    for (id, track) in tracks {
        if let Some(albums) = result.get_mut(&track.tag.artist) {
            if let Some(tracks) = albums.get_mut(&track.tag.album) {
                // Add it to existing entry if known
                tracks.push(*id);
            } else {
                // Create new key value pair
                albums.insert(track.tag.album.clone(), vec![*id]);
            }
        } else {
            // Create new key value pair
            result.insert(track.tag.artist.clone(), HashMap::new());
            result
                .get_mut(&track.tag.artist)
                .unwrap()
                .insert(track.tag.album.clone(), vec![*id]);
        }
    }
    result
}

pub fn format_artist_track(
    listing: &HashMap<String, HashMap<String, Vec<usize>>>,
    selection: (usize, usize, &HashMap<usize, usize>),
    focus: u8,
    lookup: &HashMap<usize, Track>,
    playing: Option<usize>,
) -> Vec<String> {
    let mut result = vec![];
    let (artist_ptr, album_ptr, track_ptr) = selection;
    // Gather list of artists
    let mut artists: Vec<&String> = listing.keys().collect();
    artists.sort();
    // Gather list of selected artist's albums
    let mut albums: Vec<&String> = listing[artists[artist_ptr]].keys().collect();
    albums.sort();
    // Gather years for albums
    let mut years = vec![];
    for album in &albums {
        let artist = &listing[artists[artist_ptr]];
        let album = &artist[*album];
        let track_id = album[0];
        years.push(lookup[&track_id].tag.year.to_string());
    }
    // Gather list of all tracks from this artist
    let mut tracks: Vec<usize> = vec![];
    for album in &albums {
        let this = &listing[artists[artist_ptr]][*album];
        for track in this {
            tracks.push(*track);
        }
    }
    // Format rhs of table
    let curve_bar = format!("{}╭{}", SetFg(Color::DarkBlue), SetFg(Color::Reset));
    let vertical_bar = format!("{}│{}", SetFg(Color::DarkBlue), SetFg(Color::Reset));
    for (album, year) in albums.iter().zip(years) {
        result.push(format!(
            "{} {}{} - {}{}",
            curve_bar,
            SetFg(Color::DarkBlue),
            album,
            year,
            SetFg(Color::Reset)
        ));
        let this = &listing[artists[artist_ptr]][*album];
        for track in this {
            let track_title = if Some(*track) == playing {
                format!(
                    "{}{}{}",
                    SetFg(Color::Green),
                    lookup[track].tag.title,
                    SetFg(Color::Reset)
                )
            } else {
                format!("{}", lookup[track].tag.title)
            };
            if *track == tracks[track_ptr[&artist_ptr]] {
                if focus == 0 {
                    result.push(format!("{} {}", vertical_bar, track_title,));
                } else {
                    result.push(format!(
                        "{} {}{}{}",
                        vertical_bar,
                        SetBg(Color::DarkGrey),
                        track_title,
                        SetBg(Color::Reset)
                    ));
                }
            } else {
                result.push(format!("{} {}", vertical_bar, track_title));
            }
        }
    }
    // Fill spaces
    if artists.len() > albums.len() {
        let left = artists.len() - albums.len();
        for _ in 0..left {
            result.push("".to_string());
        }
    }
    // Splice lhs of table
    let pad = find_longest(&artists);
    for (row, artist) in result.iter_mut().zip(&artists) {
        if artist == &artists[artist_ptr] {
            if focus == 0 {
                *row = format!(
                    "{}{}{} {}",
                    SetBg(Color::DarkGrey),
                    align_left(artist, pad),
                    SetBg(Color::Reset),
                    row
                );
            } else {
                *row = format!(
                    "{}{}{} {}",
                    SetFg(Color::DarkBlue),
                    align_left(artist, pad),
                    SetFg(Color::Reset),
                    row
                );
            }
        } else {
            *row = format!("{} {}", align_left(artist, pad), row);
        }
    }
    result
}

pub fn artist_tracks(
    listing: &HashMap<String, HashMap<String, Vec<usize>>>,
    artist: usize,
) -> Vec<usize> {
    let mut artists: Vec<&String> = listing.keys().collect();
    artists.sort();
    let mut albums: Vec<&String> = listing[artists[artist]].keys().collect();
    albums.sort();
    let mut result = vec![];
    for album in albums {
        for track in &listing[artists[artist]][album] {
            result.push(*track);
        }
    }
    result
}
