# Klippy

Klippy is a macOS-first clipboard manager built with Tauri v2, Rust, SolidJS, Vite, Tailwind, and Bun.

It watches clipboard text changes, stores them locally, and gives you a fast searchable UI with pin/copy/delete actions.

## Features

- Clipboard history for `text`, `url`, and `code` clips.
- Global shortcut `Cmd + Shift + V` to show/hide the app window.
- Tray icon click toggles the app window.
- Full-card click to copy a clip back to clipboard.
- Debounced search with keyboard selection (`↑` / `↓`).
- Pin/unpin clips.
- Delete single clip or `Clear All`.
- Auto-pruning with pinned protection.
- Starts at login (autostart enabled).
- Auto-minimizes when focus moves to another app.
- Close button minimizes to background (does not quit).
- Local-only storage in SQLite (WAL mode).
- Clipboard content from Klippy itself is ignored.

## Privacy Defaults

- No cloud sync.
- No clipboard content logging.
- Default denylist for known password manager app bundle IDs.
- Max stored clip payload: `256 KB`.
- Default history limit: `200` clips.

## Tech Stack

- Frontend: SolidJS + Vite + Tailwind + TypeScript
- Backend: Rust 2021 + Tauri v2
- DB: SQLite via `rusqlite`
- Package manager: Bun

## Requirements

- macOS 12.0+
- Bun
- Rust toolchain (stable, Rust 1.78+ recommended)
- Xcode Command Line Tools

Install Xcode CLT if needed:

```bash
xcode-select --install
```

## Development

Install dependencies:

```bash
bun install
```

Run in development:

```bash
bun run tauri:dev
```

If `tauri` command is not found:

```bash
bunx tauri dev
```

## Build

Create a release build:

```bash
bun run tauri:build
```

Common output paths:

- App bundle: `src-tauri/target/release/bundle/macos/Klippy.app`
- Other bundles (for example DMG/PKG): `src-tauri/target/release/bundle/`

## Memory Measurement (Release)

Target for optimization: **Klippy main process idle memory <= 45 MB**.

Measure in release mode only:

1. Build release with `bun run tauri:build`.
2. Launch `Klippy.app` from `/Applications` (or the release bundle output).
3. Let it idle for ~30 seconds with the window hidden/minimized.
4. Check Activity Monitor and measure the `Klippy` main process.

Notes:

- `tauri dev` (`tauri://localhost`) includes development overhead and is not the memory optimization target.
- WebKit helper processes may appear separately; use the main `Klippy` process for this baseline target.

## Install on macOS

1. Build with `bun run tauri:build`.
2. Move `Klippy.app` to `/Applications`.
3. Launch Klippy from Applications.

## Tests and Checks

Frontend checks:

```bash
bun run lint
bun run test
bun run build
```

Rust checks:

```bash
cd src-tauri
cargo test
cargo clippy --all-targets -- -D warnings
```

## Project Structure

- `src/`: SolidJS app (UI, state, components)
- `src-tauri/`: Rust backend, clipboard watcher, DB, and Tauri runtime

## Current UX Notes

- Smart Paste is not exposed in the current UI.
- `Stop` button exits the app.

## License

MIT
