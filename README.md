[![Build m_nowplaying DLL](https://github.com/irc-js/m_nowplaying/actions/workflows/ci.yml/badge.svg)](https://github.com/irc-js/m_nowplaying/actions/workflows/ci.yml)
# m_nowplaying DLL for mIRC / AdiIRC

## Requirements

- mIRC v6.10 or later (or AdiIRC)
- Windows 10 or later

## Introduction

This DLL provides access to the Windows Media API for mIRC and AdiIRC scripts, allowing you to retrieve information about the currently playing media track.

## Building

### For mIRC and AdiIRC 32 Bit
To build the DLL for AdiIRC (x86) and mIRC, run the following command:
```sh
cargo build --target i686-pc-windows-msvc --release
```

### For AdiIRC 64 Bit
To build the DLL for AdiIRC (x86_64), run the following command:
```sh
cargo build --target x86_64-pc-windows-msvc --release
```

### For AdiIRC ARM
To build the DLL for AdiIRC (aarch64), run the following command:
```sh
cargo build --target aarch64-pc-windows-msvc --release
```

## Usage

### Core Functions

- `wait_for_media`: Starts listening for media events. Must be called with `$dllcall`. It does not block; instead, it will callback once a media change has been detected. 
- `halt`: Stops listening for media events and unblocks any waiting calls.

### Track Information Functions

These functions return information about the currently playing track (or an empty string if not available):

- `title`: Track title
- `artist`: Track artist
- `albumartist`: Album artist
- `albumtitle`: Album title
- `genres`: Track genres (comma-separated)
- `playbacktype`: Playback type (Music, Video, Image, Unknown)
- `subtitle`: Track subtitle
- `tracknumber`: Track number
- `albumtrackcount`: Total number of tracks in the album
- `thumbnail`: Path to a temporary file containing the track's thumbnail image (if available)

### Version

- `version`: Returns the DLL version and build information.

## Example (mIRC)

```msl
//noop $dllcall(m_nowplaying.dll, callback_alias, wait_for_media, $null)
```

mIRC will call the specified alias (`callback_alias`) in your remotes once media has been detected.

```msl
alias callback_alias {
    echo -a Now playing: $dll(m_nowplaying.dll, title, $null)
}
```

## Notes

- This DLL requires mIRC v6.10 or later due to requiring $dllcall support.
- Only one listener is supported at a time.
- The thumbnail is written to a temporary file on demand.
