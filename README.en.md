<p align="center"><img src="assets/icon/telima.png" width="128" alt="telima icon: a pixel teddy bear hugging a container"></p>

# telima

A desktop app for [Colima](https://github.com/abiosoft/colima) profiles and their containers, images and volumes on macOS and Linux, from a window and a tray icon. Written in Rust with [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui).

[한국어](README.md)

> Status: v0.1.1. Early: few features, and they may change. The app is not signed or notarised.

The app's interface is in Korean only.

## Features

- Colima profiles: list, create, edit, start, stop, restart, delete (optionally with the data disk)
- Start/stop/restart progress steps and log, elapsed time, uptime
- Containers: list (state, ports, start time), start, stop, restart, remove, remove all exited
- Container logs: timestamps, follow, search
- Images and volumes: list and prune unused; prune the Colima cache
- Colima version check at startup with a notice banner
- Menu bar (macOS) / system tray (Linux) icon; the app keeps running after the window is closed
- Follows the system light/dark appearance

Details (Korean): [feature spec](docs/features.md), [usage guide](docs/guide.md).

## Install

Download from [GitHub Releases](https://github.com/bear-engineer/telima-desktop/releases). Each release also carries `SHA256SUMS-*.txt` checksum files.

### macOS (Apple Silicon)

1. Unzip `telima-v0.1.1-macos-arm64.zip` to get `telima.app` and move it to `/Applications`.
2. The app is unsigned, so macOS blocks the first launch. Either:
   - In Finder, right-click (or Control-click) `telima.app` → Open → Open. If your macOS version refuses, use System Settings → Privacy & Security → "Open Anyway".
   - Or remove the quarantine attribute:

     ```sh
     xattr -dr com.apple.quarantine /Applications/telima.app
     ```

Needs macOS 13 or later (`LSMinimumSystemVersion` in `scripts/bundle-macos.sh`).

### Linux (x86_64)

`telima-v0.1.1-linux-x86_64.tar.gz` contains the `telima` binary, the desktop entry `io.github.bear_engineer.telima.desktop`, the icon directory `hicolor/` and the README.

```sh
tar -xzf telima-v0.1.1-linux-x86_64.tar.gz
cd telima-v0.1.1-linux-x86_64
install -Dm755 telima ~/.local/bin/telima
install -Dm644 io.github.bear_engineer.telima.desktop ~/.local/share/applications/io.github.bear_engineer.telima.desktop
mkdir -p ~/.local/share/icons
cp -R hicolor ~/.local/share/icons/
```

The desktop entry runs `Exec=telima`, so `~/.local/bin` must be on `PATH`. The tray icon needs a panel with StatusNotifierItem support; GNOME needs the AppIndicator extension ([docs/tray.md](docs/tray.md), Korean).

## Requirements

- Colima 0.10.3 or later; tested with 0.10.3. Older versions get the "Colima 업데이트 필요" (update needed) banner, newer ones the "검증되지 않은 Colima 버전" (untested version) banner (`MIN_SUPPORTED`, `LAST_TESTED` in `crates/telima-core/src/version.rs`).
- Only the docker profile runtime is supported. Containers, images and volumes are read through the Docker Engine API on `~/.colima/<profile>/docker.sock`, so the docker CLI is not needed.
- `colima` is looked up on `PATH`, then in `/opt/homebrew/bin`, `/usr/local/bin`, `~/.local/bin`, `/home/linuxbrew/.linuxbrew/bin`.

## Build from source

Requirements:

- Rust 1.98.1 (pinned in `rust-toolchain.toml`; rustup installs it)
- macOS: Command Line Tools (`xcode-select --install`). Full Xcode is not needed.
- Linux (Debian/Ubuntu): `sudo apt install clang libxkbcommon-x11-dev libwayland-dev libx11-xcb-dev libfontconfig-dev libvulkan1 libssl-dev libdbus-1-dev pkg-config`
- Colima, for running the app (not for building)

```sh
cargo run -p telima
cargo test --workspace
```

Packaging:

- macOS: `scripts/bundle-macos.sh` builds an unsigned `target/release/bundle/telima.app` with the icon. `cargo run` shows the generic Dock icon.
- Linux: copy `packaging/linux/io.github.bear_engineer.telima.desktop` to `~/.local/share/applications/` and the contents of `assets/icon/hicolor/` into `~/.local/share/icons/hicolor/`. The window reports the same app id (`APP_ID` in `crates/telima-ui/src/lib.rs`), so the desktop shows the icon.
- Icons: `assets/icon/telima.png` (1024 px) is the only master. `scripts/icons.sh` (macOS) regenerates the sizes and `telima.icns` from it. See [docs/icon.md](docs/icon.md) for where it came from.
- Releases: pushing a `v*` tag runs `.github/workflows/release.yml`, which builds the macOS zip and the Linux tarball and uses the matching section of [CHANGELOG.md](CHANGELOG.md) as release notes.

## Layout

Dependencies point inward only; `scripts/check_layers.py` enforces it in CI.

| Crate | Role |
|---|---|
| `telima-core` | domain entities, rules, port traits |
| `telima-app` | use cases, state, view models, Korean copy |
| `telima-colima` | Colima CLI adapter |
| `telima-docker` | Docker Engine API adapter (bollard) |
| `telima-tray` | menu bar / system tray adapter |
| `telima-ui` | GPUI views |
| `telima` | binary; wires everything together |

## Docs (Korean)

- [docs/features.md](docs/features.md): feature spec
- [docs/guide.md](docs/guide.md): usage guide
- [docs/tray.md](docs/tray.md): tray icon
- [docs/dev.md](docs/dev.md): development environment variables
- [CHANGELOG.md](CHANGELOG.md): changes
- [CONTRIBUTING.md](CONTRIBUTING.md)

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at your option.
