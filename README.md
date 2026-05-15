# RustySpeedTest
[![CI](https://github.com/Saket-Upadhyay/RustySpeedTest/actions/workflows/ci.yml/badge.svg)](https://github.com/Saket-Upadhyay/RustySpeedTest/actions/workflows/ci.yml) ![Rust Badge](https://img.shields.io/badge/Rust-orange?logo=Rust) ![Static Badge](https://img.shields.io/badge/-Tahoe_26.4.1-black?logo=macOS) ![Static Badge](https://img.shields.io/badge/Fedora%2044-black?logo=fedora)


![Logo](./doc/rusty-fast.png)

A small, CLI/TUI internet speed test wrapper for fast.com.

Quick start:

```bash
cargo run --
```

Force non-interactive mode:

```bash
cargo run -- --no-tui
```

Options:
- `-c, --connections <N>` (default: 4)
- `-d, --duration <S>` (seconds, default: 8)
- `--no-tui` (force CLI)
