# RustySpeedTest
A small, terminal-first download/upload speed test using fast.com.

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
