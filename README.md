# Dockpit

[English](README.md) · [Español](README.es.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/built%20with-Rust-orange.svg)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20macOS-lightgrey.svg)](#requirements)

**A fast, flicker-free terminal UI for managing Docker containers.**

Dockpit is a single static binary that gives you a live, dual-panel view of your
containers — stream logs with infinite scroll and search, watch real-time stats,
and run the full container lifecycle without leaving the terminal. It's built in
Rust on top of [ratatui](https://github.com/ratatui/ratatui),
[bollard](https://github.com/fussybeaver/bollard) and Tokio, following the Elm
architecture (unidirectional `Message → Update → View`) for a UI that never
flickers and never leaks.

![Dockpit — live container management in the terminal](docs/assets/overview.gif)

## Features

- **Dual-panel TUI** — container list on the left, logs or stats on the right, updated in real time with zero flicker.
- **Full lifecycle control** — start, stop, restart, pause, unpause and remove containers from an in-app menu.
- **Live logs with infinite scroll** — scroll past the visible buffer and older history is fetched on demand, paginated by timestamp.
- **In-log search** — `/` to search, `n`/`N` to jump between matches, highlighted inline (ASCII case-insensitive).
- **Log-level filtering** — cycle `All → Error → Warn → Info → Debug → Trace` with `Tab`; scroll, scrollbar and title all respect the filter.
- **Copy & export** — copy presets or the full log to the clipboard, export to a timestamped file, or print to the terminal for manual selection.
- **Real-time stats** — CPU %, memory usage/limit, network and block I/O.
- **Exec & inspect** — run commands inside a container from the CLI.
- **SSH-friendly clipboard** — uses native tools locally and **OSC 52** over SSH, so copying works even on a remote host with no X11.
- **CLI mode** — scriptable subcommands (`list`, `start`, `stop`, `restart`, `logs`, `stats`, `exec`) alongside the interactive TUI.

## Feature tour

### Live logs with color-coded levels

Stream a container's logs in real time — `ERROR`/`WARN`/`INFO`/`DEBUG` are
color-coded, and scrolling past the top transparently loads older history
(infinite scroll).

![Live logs with color-coded levels](docs/assets/logs-levels.gif)

### Search inside logs

Press `/`, type a query, and jump between highlighted matches with `n`/`N`.

![In-log search](docs/assets/search.gif)

### Filter by log level

Cycle the visible level with `Tab` — the title, scrollbar and line count all
follow the filter.

![Filter by log level](docs/assets/filter.gif)

### Real-time stats

Switch to the stats view with `S` for live CPU, memory, network and block I/O.

![Real-time container stats](docs/assets/stats.gif)

### Container operations & clipboard

`D` opens the lifecycle menu (start / stop / restart / pause / unpause / remove);
`C` opens the clipboard menu (copy presets, export to file, or print for SSH).

![Docker operations and clipboard menus](docs/assets/menus.gif)

## Installation

### From source (recommended)

Requires a recent stable [Rust toolchain](https://rustup.rs/).

```bash
git clone https://github.com/Corpy-ai/dockpit.git
cd dockpit
cargo build --release
./target/release/dockpit
```

### With cargo install

```bash
cargo install --git https://github.com/Corpy-ai/dockpit
```

### Prebuilt binary

Download the binary for your platform from the
[Releases](https://github.com/Corpy-ai/dockpit/releases) page and place it on your `PATH`:

```bash
sudo install -m 0755 dockpit /usr/local/bin/dockpit
```

## Usage

### Interactive TUI (default)

```bash
dockpit
```

### CLI commands

```bash
dockpit list [--all]                     # list containers (default: running only)
dockpit start   <container>              # start a container
dockpit stop    <container>              # stop a container
dockpit restart <container>              # restart a container
dockpit logs    <container> [--lines N] [--follow]
dockpit stats   [container]              # resource stats
dockpit exec    <container> <command...> # run a command inside a container
```

## Keyboard shortcuts (TUI)

| Keys | Action |
|------|--------|
| `↑` / `↓` or `j` / `k` | Move selection / scroll logs |
| `←` / `→` or `h` | Move focus between the container and log panels |
| `1`–`9` (type a number) | Jump directly to container N |
| `L` | Logs view |
| `S` | Stats view |
| `F` | Toggle full-screen (expanded) logs |
| `/` | Search in logs (`Enter` jump, `n`/`N` next/prev, `Esc` cancel) |
| `Tab` | Cycle log-level filter (All/Error/Warn/Info/Debug/Trace) |
| `D` | Docker operations menu |
| `C` | Clipboard menu |
| `R` | Restart selected container |
| `PageUp` / `PageDown` | Scroll logs by 10 lines |
| `Home` / `End` | Jump to start / end of logs |
| `Q` | Quit |

**Docker operations menu (`D`):** `1` Start · `2` Stop · `3` Restart · `4` Pause · `5` Unpause · `6` Remove · `Esc` close.

**Clipboard menu (`C`):** `1` Last 100 lines · `2` Last 500 lines · `3` Visible lines · `4` From current position to end · `5` All logs · `6` Export to file · `7` Print to terminal · `Esc` close.

## Copying logs over SSH

When you run Dockpit on a remote host over SSH, the remote machine's clipboard
tools (`xclip`/`wl-copy`) can't reach *your* clipboard. There are three options:

1. **Print to terminal — option `7` (works in any terminal).** The TUI drops back
   to the normal scrollback and prints the logs (respecting the active level
   filter). Select with the mouse and copy with `Ctrl+Shift+C`; `Enter` returns to
   the TUI. The most reliable option for large volumes.
2. **Native mouse selection.** Dockpit never captures the mouse, so you can always
   select the visible logs and copy with `Ctrl+Shift+C`. Use expanded logs (`F`)
   for a cleaner selection. Limited to what's on screen.
3. **OSC 52 — clipboard menu options `1`–`6`.** These emit an OSC 52 escape
   sequence that the *local* terminal intercepts and writes to your clipboard,
   straight through SSH. Auto-selected over SSH; force it with
   `DOCKPIT_CLIPBOARD=osc52` (or `=local` for the native backend).

> ⚠️ **GNOME Terminal / VTE does not support OSC 52** (Tilix, xfce4-terminal,
> Ptyxis and Black Box are VTE too). Use option `7` or native selection there.
> OSC 52 works in kitty, alacritty, wezterm, foot, ghostty, iTerm2 and Konsole
> (with "allow programs to write to the clipboard" enabled). tmux/screen
> passthrough is wrapped automatically (tmux needs `allow-passthrough on`,
> default since 3.3). Practical OSC 52 limit is ~100 KB — for more, use option
> `7` or **Export** (`6`).

Verify your terminal:

```bash
printf '\033]52;c;%s\007' "$(printf 'osc52-works' | base64 -w0)"
# Paste (Ctrl+V): if "osc52-works" appears, your terminal supports OSC 52.
```

## Requirements

- **Docker** running and reachable (the user must be in the `docker` group or use `sudo`).
- A terminal with 256-color support.
- Optional: `xclip`/`xsel` (X11) or `wl-clipboard` (Wayland) for the native clipboard backend on Linux. macOS uses `pbcopy` out of the box.

## Architecture

```
src/
├── main.rs              # CLI parsing (clap) + entry point
├── app/                 # Elm architecture
│   ├── message.rs       # Message enum, LogEntry/LogLevel, log parsing
│   ├── state.rs         # AppState and all view/navigation/menu state
│   ├── update.rs        # update(): Message → State → Effects (pure)
│   └── effects.rs       # EffectRunner: runs side effects (Docker, clipboard)
├── docker/mod.rs        # Docker API client (bollard): list/start/stop/logs/stats/exec
├── ui/
│   ├── mod.rs           # Event loop, terminal setup, clipboard backend detection
│   └── view.rs          # ratatui rendering (panels, menus, highlighting, scrollbar)
└── utils/
    ├── clipboard.rs     # Multi-platform clipboard (wl-copy/xclip/xsel/pbcopy/arboard)
    └── osc52.rs         # OSC 52 clipboard over SSH
```

State is immutable; every change flows through the pure `update()` function,
which returns the next state plus a list of `Effect`s. The `EffectRunner`
executes those effects on Tokio tasks (Docker API calls, log/stat streams,
clipboard) and feeds results back as `Message`s. All I/O is asynchronous and
non-blocking, and the UI only redraws when something actually changes.

Developer notes about specific fixes and optimizations live in
[`docs/dev-notes/`](docs/dev-notes/).

## Troubleshooting

**"Failed to connect to Docker daemon"**

```bash
sudo systemctl start docker          # make sure the daemon is running
sudo usermod -aG docker "$USER"      # then log out and back in
```

**Clipboard doesn't work locally**

```bash
sudo apt-get install xclip           # Debian/Ubuntu (or wl-clipboard on Wayland)
sudo dnf install xclip               # Fedora
```

For SSH, see [Copying logs over SSH](#copying-logs-over-ssh).

## Contributing

Contributions are welcome. Open an issue to discuss a change, or send a pull
request: fork the repo, create a feature branch, commit your changes, and open a
PR against `main`.

## License

[MIT](LICENSE) © Corpy
