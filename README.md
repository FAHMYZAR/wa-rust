# `wa-rust` // WhatsApp Automation Core

> Rust-powered WhatsApp Web bot with a green terminal heartbeat.

```text
[ ONLINE ]  wa-rust
[ ENGINE ]  Rust + Tokio + whatsapp-rust
[ LINK ]    WhatsApp Linked Device
[ MODE ]    self-hosted / container-ready
```

`wa-rust` is a lightweight, always-on WhatsApp bot built in Rust. It connects as a linked device, stores its session in SQLite, and provides general, group, and media commands without a browser or official Cloud API.

## Features

- Fast async runtime powered by Tokio
- QR pairing and optional 8-digit pairing code
- Persistent WhatsApp session in `whatsapp.db`
- SQLite-backed group warning and contact data
- Media tools powered by FFmpeg, ImageMagick, and WebP tools
- Automatic cleanup of generated files in `disk/tmp`
- Docker and Portainer deployment support
- Environment-based command prefixes and API keys

## Quick Start

### Native

```bash
cargo run
cargo run -- --phone 6281234567890
```

Scan the QR from the terminal with WhatsApp → Settings → Linked Devices.

### Release build

```bash
cargo build --release
./target/release/wa-rust
```

## Portainer / Docker

The included `Dockerfile` builds the Rust release inside Alpine. Its runtime image includes the media dependencies required by the bot.

1. Open **Stacks** → **Add stack** in Portainer.
2. Choose **Repository** and enter this repository URL.
3. Set the compose path to `docker-compose.yml`.
4. Add variables manually in **Environment variables**.
5. Deploy the stack.
6. Open **Containers** → **wa-rust** → **Logs** to scan the QR.

Recommended values:

```text
OWNER_NUMBER=6281234567890
OWNER_PREFIX=/
USER_PREFIX=.
RUST_LOG=info
TZ=Asia/Jakarta
PAIR_PHONE=
```

For an 8-digit pairing code, set `PAIR_PHONE` to the bot phone number using digits and country code only. Both the pair code and terminal-style QR appear in the Portainer container logs.

The named volume `wa_rust_data` persists:

```text
whatsapp.db
warns.db
disk/
```

Do not remove the volume unless the bot should log out and pair again.

## Configuration

Copy `.env.example` for local Compose use, or enter values directly in Portainer. Never commit actual API keys or `.env` files.

| Variable | Required | Purpose |
|---|---:|---|
| `OWNER_NUMBER` | Yes | Owner phone number, digits only |
| `OWNER_PREFIX` | No | Owner command prefix, default `/` |
| `USER_PREFIX` | No | User command prefix, default `.` |
| `PAIR_PHONE` | No | Phone number for pairing-code flow |
| `RUST_LOG` | No | Log level, default `info` |
| `OCR_API_KEY` | No | OCR service key |
| `RESITA_API_KEY` | No | Image enhancement service key |
| `LOLHUMAN_API_KEY` | No | Hug/GIF service key |
| `RMBG_FUZZ` | No | Background-removal fuzz value |

## Command Groups

- **General** — ping, help, translation, earthquake information, hugs, logs
- **Group** — tagging, moderation, admin tools, warnings, links, group settings
- **Media** — stickers, quote stickers, OCR, conversion, enhancement, memes, background removal

Send the configured user prefix followed by `help` to view the dynamic command menu.

## Project Layout

```text
src/
├── main.rs
├── core/           # Feature contract, registry, dispatcher
├── features/       # General, group, and media commands
└── utils/          # Media, contacts, groups, QR, HTTP, SQLite helpers
```

## License

Private self-hosted project. Review dependency licenses before redistribution.
