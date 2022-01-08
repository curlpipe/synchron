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

# Notes
This takes inspiration from `termusic` and `cmus`.
