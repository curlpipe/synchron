# Synchron
> A terminal music player

# Installation
This requires gstreamer as well as it's plugins and development libraries.
You will also need a dbus session running in order to use the MPRIS capabilities.

```sh
git clone https://github.com/curlpipe/synchron
cd synchron
cargo build --release
sudo cp target/release/synchron /usr/bin/synchron
```

You can then start synchron with the `synchron` command.

# Usage
Once opened, you will be greeted with a prompt.
This prompt will be where you can type commands to control the player.

## Command line
Please run `synchron -h` for more help information.

There are two modes in synchron. CLI and TUI. CLI is a bit more powerful at the moment,
but TUI is a closer experience to applications like Spotify as it provides an interactive
way to manage and play your music.

TUI is the default mode and can be triggered simply by running `synchron`.

CLI mode can be triggered by explicitly stating the `-c` flag: `synchron -c`.

Note: TUI mode is not fully functional yet, and you may have to use the command mode
to properly edit your library. You can trigger the command mode from within TUI mode
by using the <kbd>;</kbd> or <kbd>:</kbd> keys.

See below for a list of default commands and key bindings that you'll need to know in 
order to use synchron.

## TUI keyboard shortcuts
| Key                                | What it does                              |
|------------------------------------|-------------------------------------------|
| <kbd>q</kbd>                       | Quits the application and stops playback. |
| <kbd>t</kbd>                       | Toggle play / pause.                      |
| <kbd>x</kbd>                       | Stop music playback.                      |
| <kbd>c</kbd>                       | Start music playback.                     |
| <kbd>v</kbd>                       | Pause music playback.                     |
| <kbd>Enter</kbd>                   | Play selected track in library.           |
| <kbd>d</kbd>                       | Delete selected track from library.       |
| <kbd>Up</kbd>                      | Move selection up.                        |
| <kbd>Down</kbd>                    | Move selection down.                      |
| <kbd>Ctrl</kbd> + <kbd>Up</kbd>    | Move selection to top.                    |
| <kbd>Ctrl</kbd> + <kbd>Down</kbd>  | Move selection to bottom.                 |
| <kbd>Alt</kbd> + <kbd>Up</kbd>     | Move selected track in library upwards.   |
| <kbd>Alt</kbd> + <kbd>Down</kbd>   | Move selected track in library downwards. |
| <kbd>Left</kbd>                    | Seek forward 5 seconds.                   |
| <kbd>Right</kbd>                   | Seek backward 5 seconds.                  |
| <kbd>Ctrl</kbd> + <kbd>Right</kbd> | Move to the next track.                   |
| <kbd>Ctrl</kbd> + <kbd>Left</kbd>  | Move to the previous track.               |
| <kbd>l</kbd>                       | Toggle loop status.                       |
| <kbd>h</kbd>                       | Toggle shuffle status.                    |
| <kbd>m</kbd>                       | Toggle mute.                              |
| <kbd>Shift</kbd> + <kbd>Up</kbd>   | Volume up.                                |
| <kbd>Shift</kbd> + <kbd>Down</kbd> | Volume down.                              |
| <kbd>;</kbd> OR <kbd>:</kbd>       | Open command mode within TUI mode.        |
| <kbd>1</kbd>                       | Go to simple library pane.                |
| <kbd>2</kbd>                       | Go to empty pane.                         |
| <kbd>4</kbd>                       | Go to file browser.                       |

## CLI mode commands
| Command                     | What it does                                             |
|-----------------------------|----------------------------------------------------------|
| open [id]                   | Opens the track from the library ID. Use `play` to play. |
| queue [id]                  | Adds the track from the library ID to the queued tracks. |
| clear                       | Clear the queue and stop playback.                       |
| status                      | Gets the metadata and position of the track.             |
| toggle                      | Plays if paused, pauses if playing.                      |
| play                        | Play the track.                                          |
| pause                       | Pause the track.                                         |
| stop                        | Stop the track and reset to beginning.                   |
| next                        | Moves to the next track in the queue.                    |
| prev                        | Moves to the previous track in the queue.                |
| loop off                    | Turn off the loop feature.                               |
| loop track                  | Loop the current playing track.                          |
| loop playlist               | Loop the current playlist / album.                       |
| loop get                    | Get the current loop status.                             |
| shuffle on                  | Turn on the shuffle feature.                             |
| shuffle off                 | Turn off the shuffle feature.                            |
| shuffle get                 | Get the current shuffle status.                          |
| volume up                   | Turn the volume up (by 0.3).                             |
| volume down                 | Turn the volume down (by 0.3).                           |
| volume set [volume]         | Set the volume on a scale of 0.0 and upwards.            |
| volume get                  | Get the current volume level.                            |
| volume reset                | Reset the volume to 1.0                                  |
| position set [time]         | Set the position to a position in seconds.               |
| position get                | Get the position and duration of the track.              |
| seek backward               | Seek back 5 seconds.                                     |
| seek forward                | Seek forwards 5 seconds.                                 |
| open playlist [name]        | Opens the specified playlist, use `play` to play .       |
| library                     | List all tracks in the library.                          |
| library add [file]          | Add a track to the library.                              |
| library remove [id]         | Remove a track from the library by its ID.               |
| playlist add [name] [id]    | Add a track at the library ID to the playlist.           |
| playlist remove [name] [id] | Remove a track from a playlist (by playlist index).      |
| playlist                    | List all playlists.                                      |
| playlist [name]             | List tracks on the specified playlist.                   |
| playlist delete [name]      | Delete a specified playlist.                             |
| playlist rename [old] [new] | Rename a specified playlist to a new name.               |
| tag title [id] [title]      | Set the title of a track by its ID.                      |
| tag album [id] [album]      | Set the album of a track by its ID.                      |
| tag artist [id] [artist]    | Set the artist of a track by its ID.                     |
| tag year [id] [year]        | Set the year of a track by its ID.                       |
| tag update [id]             | Reread the tag from a track by its ID.                   |
| tag [id]                    | List the tag of a track by its ID.                       |
| exit                        | Exit the player.                                         |

## What is the library?
The library is the list of tracks remembered by the player to play. You can use the `library` command to see the list of all the tracks in the library and their corresponding IDs. The IDs can be used in the open, queue and playlist commands. To add tracks into the library see the `library add` command in the table above.

# Configuration
At the moment, configuration is only very basic. It uses the `ron` format.
You can find an example config file at `synchron.ron`.

Firstly, it will check at `~/.config/synchron.ron` for a configuration file,
if not found, it will check at `./synchron.ron` (the current directory).
Otherwise, it will use the default configuration.

As of yet, there is only one configuration option:
- `prompt` - A string that determines how the prompt looks for typing in commands.

# Notes
This takes inspiration from `termusic` and `cmus`.
