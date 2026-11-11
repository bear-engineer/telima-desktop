# telima

A desktop app for [Colima](https://github.com/abiosoft/colima) on macOS and Linux, written in Rust with [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui).

> Status: early development. Nothing is usable yet.

## Goal

Do the daily Colima and container work without the terminal:

- create, start, stop, edit and delete Colima profiles, with live progress
- list containers, follow logs, restart and remove them
- see and clean up images and volumes

A menu bar item is planned for a later version.

## Build

Requirements:

- Rust 1.98.1 (pinned in `rust-toolchain.toml`; rustup installs it)
- macOS: Command Line Tools (`xcode-select --install`). Full Xcode is not needed.
- Linux (Debian/Ubuntu): `sudo apt install clang libxkbcommon-x11-dev libwayland-dev libx11-xcb-dev libfontconfig-dev libvulkan1 libssl-dev libdbus-1-dev pkg-config`
- Colima installed and on `PATH` (for running the app, not for building)

```sh
cargo run -p telima
cargo test --workspace
```

## Layout

Dependencies point inward only; `scripts/check_layers.py` enforces it in CI.

| Crate | Role |
|---|---|
| `telima-core` | domain entities, rules, port traits |
| `telima-app` | use cases, state, view models |
| `telima-colima` | Colima CLI adapter |
| `telima-docker` | Docker Engine API adapter (bollard) |
| `telima-ui` | GPUI views |
| `telima` | binary; wires everything together |

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at your option.
