# Contributing

- Write the failing test first, then the code that makes it pass.
- Keep one definition per fact (constants, rules, versions live in one place).
- Respect the layer rules in `scripts/check_layers.py`; CI fails on a violation.
- Never commit secrets. Enable the hook once: `git config core.hooksPath .githooks` (needs `gitleaks`).
- Tests that change Colima state must use a dedicated `telima-test-*` profile, never `default`.
- Before opening a PR: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
