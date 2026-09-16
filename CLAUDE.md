# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

wa-rust: a WhatsApp bot in Rust built on the `whatsapp-rust` crate (v0.7) using the WhatsApp Web multidevice protocol (Baileys/whatsmeow-inspired). Connects as a linked device (QR or pair code).

## Commands

```bash
cargo run                      # pair via QR
cargo run -- --phone 62812...  # pair via 8-digit code (+ QR concurrent)
cargo build
cargo test                     # single test: cargo test <name>
cargo clippy -- -D warnings
cargo fmt --check
```

Termux/Android specifics:
- `.cargo/config.toml` redirects `target-dir` to `/data/data/com.termux/files/home/.targets/wa-rust` because `/storage/emulated` is `noexec`.
- Use the Termux `pkg` rust (`/data/data/com.termux/files/usr/bin/cargo`). `~/.zshenv` sets this first.
- `whatsapp-rust`'s default `simd` feature requires nightly; `Cargo.toml` keeps `default-features = false` and omits `simd`.

## Architecture (OOP / Feature Registry Pattern)

```text
src/
├── main.rs              # Startup, QR/pair auth, event loop, shutdown
├── config.rs            # Config (OWNER_NUMBER, OWNER_PREFIX, USER_PREFIX)
├── core/
│   ├── mod.rs
│   ├── feature.rs       # `Feature` trait (name, description, category, execute), `CommandContext`, `Category` enum
│   ├── registry.rs      # `FeatureRegistry` (hash map by name/alias, category lookups)
│   └── dispatcher.rs    # `Dispatcher` (handles prefix stripping, owner/group guards, dispatch)
├── features/
│   ├── mod.rs           # `build_registry()` registers all active features
│   ├── general/         # ping, start, help, gempa, tr, hug, logs
│   ├── group/           # tagall, hidetag, grouplink, add, kick, promote, demote, warn/unwarn/warnlist
│   └── media/           # sticker, tourl, rvo, ocr, toimg, smeme, brat, bratvid, q, triger, hdsw, remini, rmbg
└── utils/
    ├── mod.rs
    ├── group.rs         # Group metadata, admin checks, target JID resolution
    ├── http.rs          # Blocking ureq helpers (run inside spawn_blocking)
    ├── media.rs         # Media download/upload helpers
    ├── qr.rs            # Unicode ASCII QR renderer
    └── warn.rs          # SQLite warns tracker (warns.db)
```

See [AGENTS.md](./AGENTS.md) for the full `Feature` trait contract, `CommandContext` API, and detailed guidelines.

### Adding a new feature

1. Create `src/features/<category>/<name>.rs` implementing `Feature`.
2. Re-export in `src/features/<category>/mod.rs`.
3. Add `Arc::new(...)` into `src/features/mod.rs::build_registry()`.
4. Run `cargo clippy -- -D warnings` and `cargo fmt`.

## Conventions

- Command logic strictly in `src/features/`.
- Cross-cutting helpers (API clients, formatters, scrapers) in `src/utils/`.
- Core dispatch/lifecycle in `src/core/`.
- Blocking work (ureq HTTP, `magick`/`ffmpeg`/`dwebp` CLI calls) must run inside `tokio::task::spawn_blocking`.
- Group admin commands: use `utils::group::require_admins(ctx)` (checks sender AND bot admin).
- Temp files: prefer `/data/data/com.termux/files/usr/tmp/` (Termux), fallback `std::env::temp_dir()`.
- User replies in features are written in Indonesian (e.g. "perintah ini hanya bisa dipakai di dalam group.").
- Loading Reactions: Any reaction used as a progress/loading indicator (e.g. `⏳`, `❤️`, `🔄`) must be unreacted/cleared (`ctx.unreact().await`) as soon as the response or error has been sent. Never leave loading reactions hanging.
