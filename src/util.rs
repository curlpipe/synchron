// util.rs - common utilities for helping out around the project
use crate::track::Track;
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

pub fn track_list_display(list: &HashMap<usize, Track>) -> Vec<usize> {
    let mut keys: Vec<usize> = list.keys().copied().collect();
    keys.sort_unstable();
    keys
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
