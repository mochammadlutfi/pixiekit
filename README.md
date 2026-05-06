# Pixiekit

Local asset preparation toolkit (Rust) untuk game / animation pipeline.

3 tool inti:
- **BG Remover** — chroma key + despill + alpha erode untuk hasil AI image generation
- **Raster → Vector (SVG)** — trace PNG/JPG ke SVG via [vtracer](https://github.com/visioncortex/vtracer)
- **Video → Sprite Sheet** — extract frame video AI (Kling/Veo) jadi horizontal sprite sheet

Triple-access:
- **CLI** (`pixiekit-cli`) — untuk scripting + AI agent (Claude Code subprocess)
- **MCP server** (`pixiekit-mcp`) — native Claude Code integration via stdio
- **Web SaaS** — Nuxt 4 frontend + axum backend (`pixiekit-web-api`, Phase 5a ✅)

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
- `vectorize` — raster → SVG via vtracer (folder batch)
- `list_presets` — reserved for Phase 6, returns empty list

Stderr carries diagnostics; stdout is reserved for protocol messages.

## Web backend (Phase 5a)

REST API server (`pixiekit-web-api`) wraps the core tools for the SaaS
frontend. See [`docs/API.md`](docs/API.md) for the full contract.

```bash
PORT=8765 cargo run --release --bin pixiekit-web-api
# → listening on 0.0.0.0:8765

# Smoke test
curl http://localhost:8765/api/health
# {"status":"ok","version":"0.1.0"}
```

Configurable env: `HOST`, `PORT`, `CORS_ALLOWED_ORIGINS` (comma-separated),
`RUST_LOG`. Body limit is 100 MiB so videos fit.

## Web frontend (Phase 5b)

A Nuxt 4 dashboard wraps the three tools for non-CLI users. By default it
runs against a mock backend so you can iterate on the UI without the Rust
backend running.

```bash
cd apps/web
pnpm install
pnpm dev
# → http://localhost:3000

# To target the real Phase 5a axum backend:
VITE_PIXIEKIT_API_URL=http://localhost:8765 pnpm dev
```

See [`apps/web/README.md`](apps/web/README.md) for layout and conventions.

## Desktop app (Phase 12 — Tauri bundle)

Native desktop alternative for offline-first usage. Reuses the Nuxt frontend
(`apps/web/`) and calls `pixiekit_core` directly via Tauri commands — no
localhost server, no HTTP roundtrip.

### Prerequisites

- Rust toolchain (stable, 1.80+) — `rustup`
- pnpm (Node 20+) — `npm i -g pnpm`
- ffmpeg (for `video-to-sprite`, `audio`, `anim-preview`)
- Platform webview deps:
  - **macOS**: Xcode Command Line Tools (`xcode-select --install`)
  - **Linux**: `libwebkit2gtk-4.1-dev`, `build-essential`, `libssl-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`
  - **Windows**: WebView2 Runtime (preinstalled on Win11; otherwise via Microsoft installer)

### Run in dev mode

```bash
cd apps/web
pnpm install
pnpm tauri:dev
# → opens native window pointing at http://localhost:3000
```

### Build a release bundle

```bash
./scripts/build-desktop.sh
# → apps/web/src-tauri/target/release/bundle/
#     macos/Pixiekit.app
#     dmg/Pixiekit_0.1.0_aarch64.dmg
```

To pick specific bundles:

```bash
./scripts/build-desktop.sh --bundles dmg          # macOS .dmg only
./scripts/build-desktop.sh --bundles appimage,deb # Linux
./scripts/build-desktop.sh --bundles msi          # Windows
```

### macOS Gatekeeper note

Bundles are **not signed by default** — first-run requires Gatekeeper override
(System Settings → Privacy & Security → "Open Anyway"). Code signing +
notarization need an Apple Developer account ($99/yr); see
[Tauri signing docs](https://v2.tauri.app/distribute/sign/macos/) for setup.

### Recent paths cache

The desktop app remembers the last 10 input/output paths in
`~/.config/pixiekit/recent.json` (override via `PIXIEKIT_CONFIG_DIR`). Presets
share the same config dir under `presets/`.

## Roadmap

| Phase | Scope | Status |
|-------|-------|:------:|
| 1 | Workspace + `core::bg_remove` + CLI | ✅ |
| 2 | `core::video_to_sprite` + CLI | ✅ |
| 3 | `core::vectorize` + CLI | ✅ |
| 4 | MCP server (stdio) | ✅ |
| 5a | SaaS backend — axum REST API | ✅ |
| 5b | SaaS frontend — Nuxt 4 dashboard | ✅ |
| 6 | Presets + humanized errors | ✅ |
| 7 | Sprite Atlas Packer | ✅ |
| 8 | Image Optimizer + Multi-DPI Scaler | ✅ |
| 9 | Audio Processor | ✅ |
| 10 | Trim & Pad + SVG Optimizer | ✅ |
| 11 | 9-Slice + Animation Preview | ✅ |
| 12 | Tauri desktop bundle | ✅ |

## License

MIT — see [`LICENSE`](LICENSE).

## Project context

Pixiekit awalnya dibangun untuk asset prep pipeline **[Dom Dom World](https://github.com/mochammadlutfi/ulin)** (Flutter educational game untuk anak Indonesia umur 3-7). Tool ini generic — bisa dipake project lain dengan workflow asset AI generation + post-processing.
