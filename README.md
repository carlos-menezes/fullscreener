# fullscreener

[![crates.io](https://img.shields.io/crates/v/fullscreener.svg)](https://crates.io/crates/fullscreener)

![demo.gif](./.github/demo.gif)

A terminal app that lets you toggle any open window into borderless fullscreen and back

## Install

```powershell
cargo install fullscreener
```

Then run `fullscreener` from any terminal.

## Build

Requires the MSVC or GNU Windows toolchain (build **on** Windows, or
cross-compile with `x86_64-pc-windows-gnu` + `mingw-w64` if you're on Linux):

```powershell
cargo build --release
```

The binary ends up at `target\release\fullscreener.exe`.

## Run

```powershell
cargo run --release
```

## Controls

| Key           | Action                                              |
|---------------|------------------------------------------           |
| `↑` `↓`     | Move selection                                      |
| `Enter`       | Toggle borderless fullscreen on the selected window |
| `r`           | Refresh the window list                             |
| `Esc`     | Quit (restores every window you fullscreened)       |
