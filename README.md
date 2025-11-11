# m_nowplaying DLL for mIRC / AdiIRC

## Introduction

This repository contains a DLL for mIRC and AdiIRC, providing access to the Windows Media API.

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

### Requires: mIRC v6.10+ ($dllcall support)

To be continued...
