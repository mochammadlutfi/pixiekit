# Pixiekit

Local asset preparation toolkit (Rust) untuk game / animation pipeline.

3 tool inti:
- **BG Remover** — chroma key + despill + alpha erode untuk hasil AI image generation
- **Raster → Vector (SVG)** — trace PNG/JPG ke SVG via [vtracer](https://github.com/visioncortex/vtracer)
- **Video → Sprite Sheet** — extract frame video AI (Kling/Veo) jadi horizontal sprite sheet

Triple-access:
- **CLI** (`pixiekit-cli`) — untuk scripting + AI agent (Claude Code subprocess)
- **MCP server** (`pixiekit-mcp`) — native Claude Code integration via stdio
- **Web SaaS** (planned, Phase 5) — Nuxt 4 frontend + axum backend

## Status

🚧 **Pre-development** (2026-05-05). Lihat [`docs/PRD.md`](docs/PRD.md) untuk spec lengkap.

## Quickstart

```bash
# Prerequisites: Rust 1.80+, ffmpeg (system)
git clone https://github.com/mochammadlutfi/pixiekit
cd pixiekit
cargo build --release --workspace

# CLI examples (after build)
./target/release/pixiekit-cli bg-remove \
  --input ./raw --output ./clean --fuzz 0.35 --erode 1

./target/release/pixiekit-cli vectorize \
  --input ./clean/wave.png --output ./svg/wave.svg --mode color

./target/release/pixiekit-cli video-to-sprite \
  --input ./videos --output ./sprites --fps 8 --size 256 --chroma-key
```

## MCP server (Claude Code integration)

Pixiekit ships an MCP server (`pixiekit-mcp`) speaking JSON-RPC 2.0 over stdio.
After a release build, register it in Claude Code:

```bash
claude mcp add pixiekit -- /absolute/path/to/target/release/pixiekit-mcp
```

Tools exposed:
- `bg_remove` — chroma key + despill + erode (batch folder)
- `video_to_sprite` — ffmpeg-based frame extraction + horizontal stitch
- `vectorize` — *stub until Phase 3 lands*; use the CLI in the meantime
- `list_presets` — reserved for Phase 6, returns empty list

Stderr carries diagnostics; stdout is reserved for protocol messages.

## Roadmap

| Phase | Scope | Status |
|-------|-------|:------:|
| 1 | Workspace + `core::bg_remove` + CLI | ✅ |
| 2 | `core::video_to_sprite` + CLI | ✅ |
| 3 | `core::vectorize` + CLI | ✅ |
| 4 | MCP server (stdio) | ✅ |
| 5a | SaaS backend — axum REST API | ⏳ |
| 5b | SaaS frontend — Nuxt 4 dashboard | ⏳ |
| 6 | (Optional) Tauri desktop bundle | ⏳ |

## License

MIT — see [`LICENSE`](LICENSE).

## Project context

Pixiekit awalnya dibangun untuk asset prep pipeline **[Dom Dom World](https://github.com/mochammadlutfi/ulin)** (Flutter educational game untuk anak Indonesia umur 3-7). Tool ini generic — bisa dipake project lain dengan workflow asset AI generation + post-processing.
