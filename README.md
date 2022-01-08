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

| Command             | What it does                                  |
|---------------------|-----------------------------------------------|
| open [file]         | Opens the file specified, use `play` to play. |
| status              | Gets the metadata and position of the track.  |
| toggle              | Plays if paused, pauses if playing.           |
| play                | Play the track.                               |
| pause               | Pause the track.                              |
| stop                | Stop the track and reset to beginning.        |
| loop off            | Turn off the loop feature.                    |
| loop track          | Loop the current playing track.               |
| loop playlist       | Loop the current playlist / album.            |
| loop get            | Get the current loop status.                  |
| shuffle on          | Turn on the shuffle feature.                  |
| shuffle off         | Turn off the shuffle feature.                 |
| shuffle get         | Get the current shuffle status.               |
| volume up           | Turn the volume up (by 0.3).                  |
| volume down         | Turn the volume down (by 0.3).                |
| volume set [volume] | Set the volume on a scale of 0.0 and upwards. |
| volume get          | Get the current volume level.                 |
| volume reset        | Reset the volume to 1.0                       |
| position set [time] | Set the position to a position in seconds.    |
| position get        | Get the position and duration of the track.   |
| seek back           | Seek back 5 seconds.                          |
| seek forward        | Seek forwards 5 seconds.                      |
| exit                | Exit the player gracefully.                   |

# Notes
This takes inspiration from `termusic` and `cmus`.
