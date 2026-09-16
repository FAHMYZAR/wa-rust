# Project Developer Instructions (AGENTS.md)

## Overview
WhatsApp bot in Rust using the `whatsapp-rust` (v0.7) crate (WhatsApp Web multidevice protocol, Baileys/whatsmeow-inspired).
Connects as a linked device via terminal QR or pair code. Session persists in SQLite (`whatsapp.db`).

## Tech Stack & Dependencies
- **Rust Edition 2021** / Tokio async runtime
- **`whatsapp-rust` 0.7**: `default-features = false` (no `simd`, which requires nightly)
- **SQLite**: `whatsapp.db` for session store; `warns.db` for group warning state via `rusqlite`
- **External CLI dependencies** (used via `std::process::Command` in `spawn_blocking`): `magick` (ImageMagick), `ffmpeg`, `dwebp`

## Environment Configuration (`.env`)
- `OWNER_NUMBER` (required): Phone number of bot owner (digits only, e.g. `62812...`)
- `OWNER_PREFIX` (optional, default `/`): Prefix for owner-only commands
- `USER_PREFIX` (optional, default `.`): Prefix for standard user commands
- Optional API keys: `OCR_API_KEY`, `RESITA_API_KEY`, `RMBG_FUZZ`

## Architecture: Feature Registry Pattern

```text
src/
├── main.rs              # Startup, auth listeners (QR/pair code), event loop, shutdown
├── config.rs            # Config struct loaded from environment
├── core/
│   ├── mod.rs
│   ├── feature.rs       # `Feature` trait, `CommandContext`, `Category` enum
│   ├── registry.rs      # `FeatureRegistry` (name & alias lookup, category grouping)
│   └── dispatcher.rs    # `Dispatcher` (prefix matching, owner/group guards, execution)
├── features/
│   ├── mod.rs           # `build_registry()` registering all active commands
│   ├── general/         # Category modules implementing Feature
│   ├── group/
│   └── media/
└── utils/
    ├── mod.rs
    ├── group.rs         # Group metadata, admin verification, target JID resolution
    ├── http.rs          # Blocking HTTP helpers (ureq) for use in spawn_blocking
    ├── media.rs         # Download/upload media extraction helpers
    ├── qr.rs            # Unicode ASCII QR renderer for terminal pairing
    └── warn.rs          # SQLite warns tracker (warns.db)
```

### Key Types & Contracts
- **`Feature` trait (`src/core/feature.rs`)**:
  - `name(&self) -> &'static str`: Command name without prefix (e.g. `"ping"`, `"warn"`).
  - `description(&self) -> &'static str`: Description displayed in the dynamic help menu.
  - `usage(&self) -> &'static str`: Usage hint string.
  - `category(&self) -> Category`: Enum variant (`General`, `Owner`, `Group`, `Media`, etc.).
  - `aliases(&self) -> &'static [&'static str]`: Secondary triggers (e.g. `&["s"]` for `sticker`).
  - `owner_only(&self) -> bool` / `group_only(&self) -> bool`: Declarative access guards.
  - `async fn execute(&self, ctx: &CommandContext<'_>) -> Result<()>`: Handler execution logic.
- **`CommandContext<'a>`**: Exposes `msg` (`MessageContext`), `args` (`&str`), `is_owner`, `is_group`, and helper methods `reply()`, `reply_quoting()`, `react()`, `send()`, `group_jid()`, `sender_jid()`.

## Adding a New Command
1. Create `src/features/<category>/<command_name>.rs` implementing the `Feature` trait.
2. Export it in `src/features/<category>/mod.rs`.
3. Add `Arc::new(<category>::<command_name>::<StructName>)` inside `src/features/mod.rs::build_registry()`.
4. Run verification (`cargo check`, `cargo clippy -- -D warnings`, `cargo fmt --check`).

## Guidelines & Pitfalls
- **Command logic strictly in `src/features/`**: Do not place feature logic in `main.rs` or `core/`.
- **Loading Reaction Behavior**: Any emoji reaction used during command execution (e.g. `⏳`, `❤️`, `🔄`, etc.) functions purely as a temporary progress/loading indicator. Once the response (message, sticker, media, or error reply) is sent, the command MUST unreact / clear the reaction via `ctx.unreact().await` or `let _ = ctx.react("").await`. Never leave reactions hanging after completion.
- **Admin verification**: Use `utils::group::require_admins(ctx)` or `utils::group::is_admin` for group administration commands.
- **Blocking I/O**: Heavy operations (HTTP via `ureq`, image processing with `Command::new("magick")`, etc.) must run inside `tokio::task::spawn_blocking`.
- **Termux / Android Environment**:
  - `.cargo/config.toml` redirects `target-dir` to `/data/data/com.termux/files/home/.targets/wa-rust` because `/storage/emulated` is `noexec`.
  - When writing temporary files, prefer `/data/data/com.termux/files/usr/tmp/` with fallback to `std::env::temp_dir()`.
- **User Build Preference**: The user prefers to run build commands (`cargo build`, `cargo run`) manually unless explicitly requested. Use `cargo check` or `cargo clippy` when verifying code.

