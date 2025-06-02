# rgb - A rusty Game Boy emulator

A pure Rust implementation of Game Boy (DMG) emulation

## Features

 - [x] Cross-platform support
 - [x] GPU-accelerated graphics using [`pixels`](https://github.com/parasyte/pixels) and [`wgpu`](https://github.com/gfx-rs/wgpu)
 - [x] Audio synthesis using [`cpal`](https://github.com/RustAudio/cpal)
 - [x] Save-games synced to the local filesystem
 - [x] Configurable hotkeys
 - [ ] Savestates
 - [ ] Gameboy Color (CGB) support

## Usage

```
Usage: rgb [OPTIONS] <rom-path>

Options:
      --skip-bootrom
  -c, --config <CONFIG>  Config file [default: config.toml]
  -d, --debug            Enable debug logs
  -s, --scale <SCALE>    Scale factor [default: 3]
  -h, --help             Print help
```
