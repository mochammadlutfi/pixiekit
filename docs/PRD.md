# Pixiekit — Technical PRD

**Project:** Pixiekit (asset preparation toolkit untuk Dom Dom World / Ulin)
**Version:** 0.2 (Phase 7-11 expansion)
**Last updated:** 2026-05-05
**Owner:** Mochammad Lutfi
**Status:** Phase 1-6 done, Phase 7-11 specced
**Repo:** https://github.com/mochammadlutfi/pixiekit

---

## ⚠ Roadmap update (2026-05-05)

Keputusan setelah PRD initial ditulis:

- **Tauri di-skip** untuk MVP. GUI desktop bukan prioritas. Manual user pakai existing
  `clean-bg.html` (di `flutter/ulin/art-workshop/rive/`) sampai SaaS frontend siap.
- **Phase 2 di-rename** dari "Tauri GUI" → "Nuxt 4 SaaS Frontend" (dipindah ke Phase 5).
- **Stack frontend SaaS**: Nuxt 4 + Vue 3 Composition API (`<script setup>`).
  Backend SaaS: axum API server (Rust, reuse `core` crate).
- **Tauri** mungkin akan ditambahkan sebagai Phase 6 (post-SaaS) — tergantung kebutuhan
  user yang tidak mau internet-dependent. Tidak prioritas sekarang.

Section yang affected: §5.2 (workspace layout — `crates/gui/` di-skip), §11 (roadmap
order), §7.2 (GUI Tauri spec — defer).

Section di bawah belum di-rewrite untuk reflect perubahan ini supaya context history
tetap visible. Treat sections terkait Tauri sebagai **deferred** (bukan deleted).

---

## ⚠ Roadmap expansion (2026-05-05, v0.2)

Phase 1-6 selesai (preset save/load uniform across CLI / MCP / web + humanized errors).
Pixiekit punya 3 tool inti: BG Remover, Vectorize, Video → Sprite. Untuk completing
asset prep pipeline ke Domdom World (Flutter + Flame, kids 3-7), ditambahkan **8 tool
baru** dalam 5 phase berikutnya:

| Phase | Tools | Rationale |
|-------|-------|-----------|
| 7 | Sprite Atlas Packer | 1 draw call vs N → game perf di tablet murah |
| 8 | Image Optimizer + Multi-Resolution Scaler | App size & multi-DPI Flutter |
| 9 | Audio Processor | Kids game = banyak SFX & narasi (Flame OGG default) |
| 10 | Trim & Pad + SVG Optimizer | Pre-step Atlas Packer + post-step Vectorize |
| 11 | 9-Slice Slicer + Animation Preview | UI button (Flame NineTileBox) + review tool |

Tauri desktop bundle masih deferred — kemungkinan jadi Phase 12 atau ditinggalkan
total kalau Nuxt SaaS frontend (Phase 5b) sudah cukup.

Detail spec di §6.4-6.11, CLI di §7.1.6-7.1.13, MCP di §7.3.2, roadmap di §11.

---

## 1. Overview

### 1.1 Apa ini

**Pixiekit** adalah local desktop toolkit (Rust) untuk mempersiapkan asset visual produksi Dom Dom World (Ulin), sebuah Flutter game edukasi anak. Tool ini bukan game-runtime — murni asset prep pipeline.

3 tool inti:

1. **BG Remover** — hapus background hijau (chroma key) dari hasil generate AI image (Nano Banana, Kling, Veo).
2. **Raster → Vector (SVG)** — trace PNG/JPG ke SVG vector path untuk import ke editor vector animation (Glaxnimate, Inkscape).
3. **Video → Sprite Sheet** — extract frame dari video AI (Kling/Veo MP4) jadi horizontal sprite sheet untuk Flame engine.

### 1.2 Kenapa dibutuhkan

Workflow asset Dom Dom World saat ini terdiri dari banyak script ad-hoc:

- `clean-bg.py` (Python) — chroma key + despill + erode
- `extract-sprites-smooth.sh` (bash) — ffmpeg extract
- `stitch-sprites-smooth.sh` (bash) — ImageMagick stitch
- (manual) buka Glaxnimate / Affinity untuk vectorize satu-per-satu

Masalah:
- Multi-runtime dependency (Python + bash + ImageMagick + Inkscape)
- Tidak bisa diakses uniform oleh AI agent (Claude Code) — AI harus tau lokasi tiap script + arg conventions
- Tidak ada GUI untuk eksplorasi parameter (slider/preview), debug visual susah
- Tidak ada batch interface yang konsisten — tiap tool punya CLI berbeda

Pixiekit menggabungkan semua jadi 1 toolkit dengan 3 antarmuka uniform: GUI (manual), CLI (scripting), MCP (AI agent native).

### 1.3 Bukan apa

- **Bukan** game asset bundler — Flutter `pubspec.yaml` tetap kelola asset declaration
- **Bukan** AI image generator — input adalah hasil generate AI yang sudah ada, tool ini cuma post-process
- **Bukan** vector editor — tracing aja, fine-tuning vector tetap di Glaxnimate/Inkscape
- **Bukan** server / cloud service — fully local, no network dependencies
- **Bukan** Dom Dom-specific — generic asset prep, bisa dipake project lain (default config-nya tuned untuk Dom Dom)

---

## 2. Goals & Non-goals

### 2.1 Goals (must-have)

| ID | Goal | Acceptance |
|----|------|------------|
| G1 | Pure local execution | Tidak butuh internet setelah install. Tidak upload file ke cloud. |
| G2 | Single static binary distribution | `.app` bundle (Mac), no install ceremony |
| G3 | Konsisten interface untuk 3 tools | Semua tool punya: input dir, output dir, options, batch mode |
| G4 | Triple-access (GUI + CLI + MCP) | Same library backing 3 frontends |
| G5 | Reproducible asset pipeline | Settings bisa di-save sebagai preset (JSON), re-run identical |
| G6 | AI-friendly CLI | Subcommand structure jelas, `--help` lengkap, JSON output mode |
| G7 | Visual preview | GUI: before/after side-by-side untuk BG Remove + Vectorize |
| G8 | Batch processing | Folder-level input → folder-level output, per-file progress |

### 2.2 Non-goals (explicit out-of-scope)

- ❌ Cloud sync, multi-user, collab features
- ❌ Web-based version (no `localhost:8765` server) — desktop GUI saja
- ❌ Mobile build (iOS/Android target Tauri) — desktop saja (Mac primary, Linux/Windows nice-to-have)
- ❌ Plugin / extension system — fixed 3 tools, tidak extensible
- ❌ Asset versioning / git-LFS integration — keluar scope
- ❌ AI image generation di dalam tool — purely post-processing
- ❌ Audio editing — visual asset saja
- ❌ Real-time collaboration / comment / review feature
- ❌ Auto-update via download — user rebuild manual atau pull binary release manual

---

## 3. User personas & access modes

### 3.1 Persona 1: Solo developer (manual mode)

**User:** Mochammad Lutfi (project owner)
**Mode:** GUI
**Frequency:** ~2-5x per minggu saat asset prep phase aktif
**Skill:** Familiar dengan Affinity/Glaxnimate, ngerti chroma key + vectorize konsep, gak mau ngapal CLI flag
**Pain points yang harus diselesaikan:**
- Capek tweak fuzz threshold trial-and-error tanpa preview
- Capek copy-paste path folder
- Capek ngingat command bash mana yang dipake

**Value yang di-deliver:**
- Drag folder → set slider → preview → batch process
- Save preset "Domdom default" → klik 1 button untuk re-run dengan setting yang sama
- Visual progress bar + log per file

### 3.2 Persona 2: AI agent (automated mode)

**User:** Claude Code (atau AI agent lain)
**Mode:** CLI atau MCP
**Frequency:** Tiap ada task asset prep di workflow
**Skill:** Bisa baca `--help`, parse JSON output, run subprocess
**Pain points yang harus diselesaikan:**
- Multi-script ad-hoc bikin context window penuh untuk hapal API
- Path script gak konsisten
- Gak bisa programmatically tau hasil success/fail per file

**Value yang di-deliver:**
- Single CLI dengan subcommand jelas: `pixiekit bg-remove`, `vectorize`, `video-to-sprite`
- JSON output mode (`--json`) untuk parsing
- MCP server: structured tool definitions, autocomplete-friendly
- Idempotent: re-run produces same result given same input + settings

---

## 4. Use cases (concrete flows)

### 4.1 UC-1: Batch BG remove untuk Domdom mascot

**Trigger:** User generate 20 PNG via Nano Banana dengan green BG, mau jadi transparent.

**Manual flow (GUI):**
1. Open Pixiekit → tab "BG Remover"
2. Drag folder `~/Downloads/domdom-poses` ke "Input"
3. Set "Output" ke `~/domdom/cleaned/`
4. Tweak fuzz slider 35% sambil preview live di gambar pertama
5. Klik "Process All" → progress bar + log per file
6. Done dalam 10 detik untuk 20 file

**Automated flow (CLI):**
```bash
pixiekit-cli bg-remove \
  --input  ~/Downloads/domdom-poses \
  --output ~/domdom/cleaned \
  --fuzz 0.35 --despill --erode 1
```

**MCP flow (AI):**
```
Claude calls: bg_remove(
  input="~/Downloads/domdom-poses",
  output="~/domdom/cleaned",
  fuzz=0.35,
  despill=true,
  erode=1
)
→ Returns: {processed: 20, failed: 0, time_ms: 8500}
```

### 4.2 UC-2: Trace illustration ke SVG untuk vector animation

**Trigger:** User punya `domdom_idle.png` (PNG hasil clean BG), mau import ke Glaxnimate jadi vector untuk skeletal rig.

**Flow:**
1. GUI → tab "Vectorize"
2. Input: `~/domdom/cleaned/domdom_idle.png` (single file mode)
3. Mode: "Color" (cartoon palette preserve)
4. Live preview: render SVG output di panel kanan
5. Tweak "Smoothness" slider sampai bentuk character pas (gak terlalu blocky, gak terlalu noisy)
6. Klik "Save As..." → simpan ke `~/domdom/svg/domdom_idle.svg`

**CLI equivalent:**
```bash
pixiekit-cli vectorize \
  --input ~/domdom/cleaned/domdom_idle.png \
  --output ~/domdom/svg/domdom_idle.svg \
  --mode color --smooth 4 --corner-threshold 60
```

### 4.3 UC-3: Generate sprite sheet dari Kling video

**Trigger:** User dapat MP4 hasil Kling (4 detik, 30fps, 1080×1080, green BG), mau extract jadi 8fps sprite sheet PNG transparent untuk Flame engine.

**Flow (GUI):**
1. Tab "Video → Sprite"
2. Input: drag `wave.mp4`
3. Output: `~/ulin/assets/images/animation/`
4. Settings:
   - Target FPS: 8
   - Frame size: 256×256
   - Output format: WebP (q=90, alpha lossless)
   - Auto-chroma-key: ✓ (BG #00FF00, fuzz 35%)
5. "Generate" → progress: "Extracting 32 frames... Chroma keying... Stitching..."
6. Output: `wave.webp` (8192×256, 32 frames @ 8fps) + `wave.json` (metadata)

**CLI equivalent:**
```bash
pixiekit-cli video-to-sprite \
  --input ~/Downloads/wave.mp4 \
  --output ~/ulin/assets/images/animation/ \
  --fps 8 --size 256 --format webp \
  --chroma-key --fuzz 0.35
```

### 4.4 UC-4: AI-orchestrated asset prep pipeline

**Trigger:** User minta Claude Code: "tolong process 5 video Kling baru di `~/Downloads/kling-batch-2026-05/` jadi sprite sheet 8fps + clean BG, output ke folder asset Ulin."

Claude executes (via MCP):
```
1. video_to_sprite(
     input="~/Downloads/kling-batch-2026-05/",
     output="~/ulin/assets/images/animation/",
     fps=8, size=256, format="webp", chroma_key=true
   )
   → {processed: 5, files: [wave.webp, happy.webp, ...]}

2. update_flutter_enum(...)  # outside this tool's scope
```

---

## 5. Architecture

### 5.1 High-level

```
┌──────────────────────────────────────────────────┐
│  Pixiekit (Cargo workspace)                      │
│                                                   │
│  ┌───────────────┐  ┌───────────────┐  ┌───────┐ │
│  │  GUI (Tauri)  │  │  CLI (clap)   │  │  MCP  │ │
│  └───────┬───────┘  └───────┬───────┘  └───┬───┘ │
│          │                  │              │     │
│          └─────────┬────────┴──────────────┘     │
│                    │                              │
│           ┌────────▼────────┐                     │
│           │  core (library) │                     │
│           │  ─────────────  │                     │
│           │  bg_remove      │                     │
│           │  vectorize      │                     │
│           │  video_to_sprite│                     │
│           └─────────────────┘                     │
│                    │                              │
└────────────────────┼──────────────────────────────┘
                     │
        ┌────────────┴───────────┐
        ▼                        ▼
   ┌────────┐              ┌──────────┐
   │ image  │              │  ffmpeg  │
   │ crate  │              │  (CLI)   │
   │ vtracer│              │          │
   └────────┘              └──────────┘
```

### 5.2 Cargo workspace layout

```
pixiekit/                            # ~/domdom/pixiekit
├── Cargo.toml                        # workspace manifest
├── Cargo.lock
├── README.md
├── docs/
│   ├── PRD.md                        # this file
│   ├── ARCHITECTURE.md               # detailed crate boundaries
│   ├── CLI.md                        # CLI reference
│   └── MCP.md                        # MCP tool definitions
├── crates/
│   ├── core/                         # 📦 LIB
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                # public API
│   │       ├── bg_remove.rs
│   │       ├── vectorize.rs
│   │       ├── video_to_sprite.rs
│   │       ├── error.rs              # AppError, Result alias
│   │       └── preset.rs             # save/load preset JSON
│   ├── cli/                          # ⌨️ BIN: pixiekit-cli
│   │   ├── Cargo.toml
│   │   └── src/main.rs
│   ├── gui/                          # 🖥 BIN: pixiekit (Tauri)
│   │   ├── Cargo.toml
│   │   ├── tauri.conf.json
│   │   ├── src/main.rs               # Tauri commands
│   │   └── ui/                       # HTML/CSS/JS frontend
│   │       ├── index.html
│   │       ├── style.css
│   │       └── app.js
│   └── mcp/                          # 🔌 BIN: pixiekit-mcp
│       ├── Cargo.toml
│       └── src/main.rs
├── examples/
│   └── presets/                      # contoh preset JSON
│       ├── domdom-bg-clean.json
│       └── domdom-sprite-8fps.json
└── tests/
    ├── fixtures/                     # test images, sample videos
    └── integration_test.rs
```

### 5.3 Crate boundaries

| Crate | Type | Depends on | Purpose |
|-------|------|------------|---------|
| `core` | lib | `image`, `imageproc`, `vtracer`, `rayon`, `serde`, `anyhow`, `thiserror` | Pure logic, no I/O orchestration |
| `cli` | bin | `core`, `clap`, `serde_json`, `indicatif` | CLI argument parsing + progress bar |
| `gui` | bin | `core`, `tauri`, `serde`, `tokio` | Tauri commands, embed HTML |
| `mcp` | bin | `core`, `mcp-sdk-rs` (or custom impl), `serde_json`, `tokio` | MCP server (stdio transport) |

**Aturan dependency:**
- `core` adalah **leaf** — tidak depend pada `cli`, `gui`, atau `mcp`
- `cli`, `gui`, `mcp` hanya depend pada `core` + framework masing-masing
- Tidak boleh ada cross-dependency antara `cli`/`gui`/`mcp`

---

## 6. Tools specification

### 6.1 BG Remover

#### 6.1.1 Input

- File tunggal: `*.png`, `*.jpg`, `*.jpeg`, `*.webp` (auto-detect via magic bytes)
- Folder: process semua file dengan ekstensi di atas, recursive opsional (default flat)

#### 6.1.2 Output

- Format: PNG dengan alpha (lossless) ATAU WebP q=90 alpha lossless (configurable)
- Naming: same filename, ekstensi mengikuti format (default replace ekstensi: `wave.jpg` → `wave.png`)
- Path: dalam folder output yang ditentukan user

#### 6.1.3 Algorithm

Pipeline pixel-by-pixel:

```
For each pixel (r, g, b, a):
  # Pass 1: chroma key
  dist = sqrt((r - target_r)² + (g - target_g)² + (b - target_b)²)
  if dist <= fuzz × max_dist:
      a = 0  # transparent
      continue

  # Pass 2: despill (only on non-transparent pixels)
  if despill_enabled:
      g_new = min(g, max(r, b))   # clamp green to higher of red/blue

# Pass 3: erode (alpha channel only, N iterations)
For each iteration:
    For each pixel:
        a_new = min(a, neighbors_alpha)  # Diamond:1 (5-point: center + 4 cardinal)
```

#### 6.1.4 Parameters

| Param | Type | Range | Default | Description |
|-------|------|-------|---------|-------------|
| `target_color` | RGB hex | `#000000` - `#FFFFFF` | `#00FF00` | BG color to remove |
| `fuzz` | f32 | 0.0 - 1.0 | 0.35 | Distance threshold (% of max RGB distance) |
| `despill` | bool | true / false | true | Clamp green channel to reduce green spill |
| `erode` | u8 | 0 - 5 | 1 | Diamond:1 morphology iterations on alpha |
| `output_format` | enum | `png` / `webp` | `png` | Output format |
| `webp_quality` | u8 | 0 - 100 | 90 | Only used if output_format=webp |

#### 6.1.5 Performance target

- Single 1024×1024 PNG: <100ms
- Batch 100 files (1024×1024 each): <10 detik (parallel via rayon)

### 6.2 Raster → Vector (Vectorize)

#### 6.2.1 Input

- File tunggal atau folder: `*.png`, `*.jpg`, `*.jpeg`, `*.webp` (preferably already with transparent BG)

#### 6.2.2 Output

- Format: `*.svg` (vtracer default output)
- Naming: same stem, `.svg` extension
- Per-file size target: <500KB untuk character ukuran 1024×1024

#### 6.2.3 Algorithm

Wrapper di sekitar [`vtracer`](https://github.com/visioncortex/vtracer) (Rust crate, MIT license).

vtracer pipeline (built-in):
1. Color quantization → reduce palette
2. Pixel grouping → cluster connected pixels
3. Path tracing → polygon outline per cluster
4. Curve fitting → smooth Bezier curves
5. SVG generation

Tool ini **tidak reimplement vtracer** — cuma expose parameter & batch interface.

#### 6.2.4 Parameters

| Param | Type | Range | Default | Description |
|-------|------|-------|---------|-------------|
| `mode` | enum | `color` / `binary` | `color` | Color preserves palette, binary = B&W |
| `filter_speckle` | u32 | 0 - 128 | 4 | Discard small speckle clusters (px²) |
| `color_precision` | u8 | 1 - 8 | 6 | Color quantization bits per channel |
| `layer_difference` | u8 | 0 - 128 | 16 | Min color difference between layers |
| `corner_threshold` | u8 | 0 - 180 | 60 | Angle threshold for corner detection (deg) |
| `length_threshold` | f64 | 0.0 - 10.0 | 4.0 | Min segment length (px) |
| `splice_threshold` | u8 | 0 - 180 | 45 | Splice angle threshold (deg) |
| `path_precision` | u8 | 0 - 16 | 8 | Decimal places for SVG path coordinates |

GUI menyederhanakan ke 1 slider "Smoothness" yang map ke kombinasi `corner_threshold` + `length_threshold` + `splice_threshold`. Advanced mode expose semua parameter.

#### 6.2.5 Performance target

- Single 1024×1024 PNG → SVG: <2 detik
- Batch 20 files: <40 detik

### 6.3 Video → Sprite Sheet

#### 6.3.1 Input

- File tunggal atau folder: `*.mp4`, `*.mov`, `*.webm`
- Asumsi: video lebih dari 1 detik, single character animasi (bukan multi-shot)

#### 6.3.2 Output

- Sprite sheet horizontal: `*.png` atau `*.webp`
- Metadata JSON sibling: `<name>.json` berisi `{frame_count, frame_size, fps, total_duration_ms}`
- Naming: `<input_stem>.<ext>`

Contoh:
```
input/wave.mp4 (4.0s, 30fps, 1080×1080)
→ output/wave.webp        (32 frames horizontal @ 256px each = 8192×256)
→ output/wave.json        ({"frame_count":32,"frame_size":256,"fps":8})
```

#### 6.3.3 Algorithm

Pipeline:

```
1. ffmpeg -i input.mp4 -vf "fps={target_fps},scale={size}:{size}:flags=lanczos" \
       /tmp/<uuid>/frame_%04d.png

2. (optional) for each frame:
       bg_remove::process(frame, chroma_options) → frame_with_alpha

3. Stitch horizontal:
       output[y, x_offset + x] = frame_n[y, x] for each frame

4. Encode as PNG / WebP

5. Write metadata JSON sibling

6. Cleanup /tmp/<uuid>/
```

ffmpeg invocation via `std::process::Command`. Frame stitching via `image` crate native (no ImageMagick needed).

#### 6.3.4 Parameters

| Param | Type | Range | Default | Description |
|-------|------|-------|---------|-------------|
| `fps` | u8 | 1 - 30 | 8 | Target output FPS (lower = fewer frames) |
| `frame_size` | u16 | 64 - 1024 | 256 | Output size per frame (square, downscaled lanczos) |
| `output_format` | enum | `png` / `webp` | `webp` | Output sprite sheet format |
| `webp_quality` | u8 | 0 - 100 | 90 | If format=webp, alpha is always lossless |
| `chroma_key` | bool | true / false | false | Apply BG remove pipeline pasca-ekstraksi frame |
| `chroma_target` | RGB hex | `#000000` - `#FFFFFF` | `#00FF00` | Only if chroma_key=true |
| `chroma_fuzz` | f32 | 0.0 - 1.0 | 0.35 | Only if chroma_key=true |
| `chroma_despill` | bool | true / false | true | Only if chroma_key=true |
| `chroma_erode` | u8 | 0 - 5 | 1 | Only if chroma_key=true |

#### 6.3.5 Performance target

- Single 4-detik 1080p video → sprite sheet: <5 detik (excluding ffmpeg decode time which is ~1-2s)
- Batch 10 videos: <60 detik

### 6.4 Sprite Atlas Packer (Phase 7)

#### 6.4.1 Input

- Folder berisi PNG sprites (recursive opsional)
- Asumsi: tiap PNG sudah trimmed/sized — tool bisa optionally re-trim

#### 6.4.2 Output

- Atlas image: `<name>.png` atau `<name>.webp` (default `atlas.png`)
- Metadata sibling: `<name>.json` (Flame-compatible TexturePacker JSON Hash format)

Contoh output JSON:
```json
{
  "frames": {
    "wave_01.png": {
      "frame": {"x": 0, "y": 0, "w": 256, "h": 256},
      "rotated": false,
      "trimmed": true,
      "spriteSourceSize": {"x": 12, "y": 8, "w": 232, "h": 240},
      "sourceSize": {"w": 256, "h": 256}
    }
  },
  "meta": {
    "image": "atlas.png",
    "size": {"w": 1024, "h": 1024},
    "format": "RGBA8888",
    "scale": "1"
  }
}
```

#### 6.4.3 Algorithm

MaxRects bin packing (best-fit, no rotation untuk simplicity). Crate: `texture_packer`
(MIT) atau `crunch`.

Pipeline:
```
1. Walk input dir → collect PNG paths
2. Decode each, optionally trim transparent bbox (record offset for spriteSourceSize)
3. Sort by area (largest first) untuk packing efficiency
4. Pack into bin (max_size × max_size, MaxRects best-fit)
5. (optional) extrude N px tiap edge (anti-bleed di game runtime)
6. Compose atlas image (image crate)
7. Encode PNG/WebP
8. Write JSON metadata sibling
```

#### 6.4.4 Parameters

| Param | Type | Range | Default | Description |
|-------|------|-------|---------|-------------|
| `name` | string | - | `atlas` | Atlas basename |
| `max_size` | u16 | 256-8192 | 2048 | Max texture dimension (square bin) |
| `padding` | u8 | 0-16 | 2 | Pixel padding antar sprite |
| `extrude` | u8 | 0-4 | 1 | Edge bleed prevention (replicate edge px) |
| `power_of_two` | bool | - | true | Force POT dimension (mobile GPU friendly) |
| `trim` | bool | - | true | Auto-trim transparent border before pack |
| `format` | enum | png/webp | png | Atlas image format |
| `webp_quality` | u8 | 0-100 | 90 | If format=webp |

#### 6.4.5 Performance target

- 100 sprite @ 256×256 → atlas + JSON: <2 detik
- 500 sprite mixed sizes: <8 detik

### 6.5 Image Optimizer (Phase 8)

#### 6.5.1 Input

- File atau folder: `*.png`, `*.jpg`, `*.jpeg`, `*.webp`

#### 6.5.2 Output

- Same struktur (file in → file out, folder in → folder out preserving structure)
- Format dapat dipertahankan (`keep`) atau diubah (e.g. `png → webp`)

#### 6.5.3 Algorithm

Operasi (configurable):
- **PNG quantization** (lossless) via `oxipng` — DEFLATE re-compress + filter optimization
- **PNG → WebP** via `image` crate `webp` feature (alpha lossless preserved)
- **JPEG re-encode** via `image` crate (mozjpeg-style quality control)
- **Strip metadata** — remove EXIF, ICC profile (kecuali sRGB), comments

#### 6.5.4 Parameters

| Param | Type | Range | Default | Description |
|-------|------|-------|---------|-------------|
| `target_format` | enum | png/webp/keep | webp | Output format (`keep` = same as input) |
| `quality` | u8 | 0-100 | 90 | Untuk lossy WebP/JPEG |
| `lossless` | bool | - | false | WebP lossless mode (alpha selalu lossless) |
| `strip_metadata` | bool | - | true | Remove EXIF/comments/non-sRGB ICC |
| `optimization_level` | u8 | 0-6 | 3 | Higher = slower, smaller (oxipng level) |

#### 6.5.5 Performance target

- 100 PNG @ 1024×1024 → optimized: <30 detik (oxipng level 3)
- Average file size reduction: 30-60% (PNG→WebP), 10-20% (PNG→PNG quantized)

### 6.6 Multi-Resolution Scaler (Phase 8)

#### 6.6.1 Input

- File atau folder image (asumsi @ base scale, default 4.0×)

#### 6.6.2 Output

- Flutter mode: `<output>/1.0x/<file>`, `2.0x/<file>`, `3.0x/<file>` (sesuai konvensi
  Flutter `flutter:assets:` variant)
- Suffix mode: `<file>.png`, `<file>@2x.png`, `<file>@3x.png` (iOS/macOS style)
- Nested mode: `<output>/<scale>/<file>` (custom)

#### 6.6.3 Algorithm

```
1. Decode source (asumsikan @ base_scale)
2. For each target scale:
     factor = scale / base_scale
     new_size = (w * factor, h * factor)
     resampled = lanczos3_resize(src, new_size)
3. Write to output path sesuai naming mode
```

#### 6.6.4 Parameters

| Param | Type | Range | Default | Description |
|-------|------|-------|---------|-------------|
| `base_scale` | f32 | 1.0-8.0 | 4.0 | Source image dianggap @ this scale |
| `target_scales` | list<f32> | - | `[1.0, 1.5, 2.0, 3.0]` | Output scales |
| `naming` | enum | flutter/suffix/nested | flutter | Folder convention |
| `filter` | enum | lanczos/bilinear/nearest | lanczos | Resampling algorithm |

#### 6.6.5 Performance target

- 1 source 4096×4096 → 4 variants: <2 detik
- 50 sources → 200 output files: <60 detik

### 6.7 Audio Processor (Phase 9)

#### 6.7.1 Input

- File atau folder: `*.wav`, `*.mp3`, `*.ogg`, `*.m4a`, `*.flac`, `*.opus`

#### 6.7.2 Output

- Single audio file per input, format target (default OGG Vorbis untuk Flame compat)
- Naming: same stem, ekstensi follows `target_format`

#### 6.7.3 Algorithm

Wraps `ffmpeg` (system dep, sudah installed). Pipeline:

```
1. (optional) Trim leading/trailing silence:
       ffmpeg -af "silenceremove=stop_periods=-1:stop_duration=0.05:stop_threshold={db}dB"
2. (optional) Loudness normalize:
       ffmpeg -af "loudnorm=I={target_lufs}:TP=-1.5:LRA=11"
3. Sample rate conversion (if sample_rate != source)
4. Channel down-mix (if channels=mono and source=stereo)
5. Encode to target_format dengan bitrate_kbps (lossy formats only)
```

#### 6.7.4 Parameters

| Param | Type | Range | Default | Description |
|-------|------|-------|---------|-------------|
| `target_format` | enum | ogg/opus/mp3/wav | ogg | Flame audio default = ogg |
| `target_lufs` | f32 | -30 to 0 | -16.0 | Mobile-friendly loudness (kids volume safety) |
| `normalize` | bool | - | true | Apply LUFS normalize |
| `trim_silence` | bool | - | true | Trim leading/trailing silence |
| `silence_threshold_db` | f32 | -60 to 0 | -50.0 | dB threshold untuk silence detection |
| `sample_rate` | u32 | 8000-48000 | 44100 | Output sample rate |
| `channels` | enum | mono/stereo/keep | keep | Channel layout |
| `bitrate_kbps` | u16 | 32-320 | 128 | Untuk lossy formats |

#### 6.7.5 Performance target

- 60s audio → normalized + converted: <2 detik
- Batch 50 SFX (rata-rata 2s each): <30 detik

### 6.8 Trim & Pad (Phase 10)

#### 6.8.1 Input

- File atau folder PNG (with alpha), atau image dengan solid background color

#### 6.8.2 Output

- Same format, dimensi adjusted sesuai content bbox + padding

#### 6.8.3 Algorithm

```
1. Decode image
2. Determine content bbox:
     - If alpha-aware: scan for pixels dengan alpha > alpha_threshold
     - If bg_color specified: scan for pixels yang bukan bg_color (within tolerance)
3. Crop ke bbox
4. (optional) Pad uniform N px (transparent / specified color)
5. (optional) Force square — pad shorter dimension to match longer
6. Encode + write
```

#### 6.8.4 Parameters

| Param | Type | Range | Default | Description |
|-------|------|-------|---------|-------------|
| `alpha_threshold` | u8 | 0-255 | 1 | Pixel dianggap content jika alpha > threshold |
| `padding` | u16 | 0-256 | 0 | Uniform padding ditambah setelah crop (px) |
| `keep_square` | bool | - | false | Force output square (pad shorter dim) |
| `bg_color` | RGB hex | - | none | If set, treat sebagai transparent (no alpha needed) |
| `bg_tolerance` | f32 | 0.0-1.0 | 0.05 | Hanya jika `bg_color` set |

#### 6.8.5 Performance target

- 100 PNG @ 1024×1024 → trimmed: <5 detik

### 6.9 SVG Optimizer (Phase 10)

#### 6.9.1 Input

- File atau folder `*.svg` (biasanya output dari §6.2 Vectorize)

#### 6.9.2 Output

- Minified SVG, same name

#### 6.9.3 Algorithm

Parsing via `usvg` crate (Rust SVG parser, MIT). Operations:

```
1. Parse SVG → Tree
2. Round path coordinates ke `precision` decimal places
3. Remove metadata: <title>, <desc>, comments, editor-specific attrs (inkscape:*, etc)
4. Remove hidden elements (display:none, visibility:hidden, opacity:0)
5. Merge similar paths (same fill/stroke + adjacent)
6. Serialize: minified (no whitespace) atau pretty (debug)
```

#### 6.9.4 Parameters

| Param | Type | Range | Default | Description |
|-------|------|-------|---------|-------------|
| `precision` | u8 | 0-10 | 3 | Decimal places untuk path coordinates |
| `remove_metadata` | bool | - | true | Strip title/desc/comments/editor attrs |
| `remove_hidden` | bool | - | true | Remove display:none / opacity:0 elements |
| `merge_paths` | bool | - | true | Merge adjacent paths dengan style sama |
| `pretty` | bool | - | false | Pretty-print (untuk debugging) |

#### 6.9.5 Performance target

- 100 SVG (avg 200KB each) → optimized: <3 detik
- Average size reduction: 30-60%

### 6.10 9-Slice Slicer (Phase 11)

#### 6.10.1 Input

- Single PNG (typically UI button background, panel, dll)
- Insets: top, right, bottom, left (px)

#### 6.10.2 Output

- **Split mode**: 9 individual PNG (`tl.png`, `t.png`, `tr.png`, `l.png`, `c.png`,
  `r.png`, `bl.png`, `b.png`, `br.png`)
- **Metadata mode**: original PNG + JSON sibling (`<name>.9slice.json`)

Output JSON format (Flame `NineTileBoxComponent` compatible):
```json
{
  "image": "button.png",
  "size": {"w": 256, "h": 96},
  "slices": {"top": 16, "right": 32, "bottom": 16, "left": 32}
}
```

#### 6.10.3 Algorithm

Split mode:
```
For each of 9 regions (computed from insets):
    Crop sub-image
    Save as separate PNG
```

Metadata mode: just write JSON sibling (image untouched).

#### 6.10.4 Parameters

| Param | Type | Range | Default | Description |
|-------|------|-------|---------|-------------|
| `top` | u16 | 0-... | required | Top inset (px) |
| `right` | u16 | 0-... | required | Right inset (px) |
| `bottom` | u16 | 0-... | required | Bottom inset (px) |
| `left` | u16 | 0-... | required | Left inset (px) |
| `output_mode` | enum | split/metadata | metadata | Split = 9 files, metadata = JSON sibling |

#### 6.10.5 Performance target

- 1 PNG → metadata mode: <50ms
- 1 PNG → split mode (9 files): <200ms

### 6.11 Animation Preview Generator (Phase 11)

#### 6.11.1 Input

- Sprite sheet PNG (horizontal, dengan optional `<name>.json` sibling untuk frame metadata)
- ATAU folder berisi frame PNG (sequential numbered: `frame_0001.png`, `frame_0002.png`, ...)

#### 6.11.2 Output

- GIF, MP4, atau WebM preview file
- Default: looping GIF (paling kompatibel untuk preview di Notion / GitHub / Slack)

#### 6.11.3 Algorithm

```
1. Detect input type:
     - .png file: assume sprite sheet, read sibling JSON for frame_size if exists
     - folder: list *.png sorted numerically
2. For sprite sheet:
     Split horizontally into N frames (frame_size × height)
     Save to /tmp/<uuid>/frame_%04d.png
3. (optional) Upscale frames (nearest neighbor) untuk visibility
4. ffmpeg compose:
     ffmpeg -framerate {fps} -i frame_%04d.png -vf "scale=iw*{u}:ih*{u}:flags=neighbor" output.{ext}
5. For GIF: add palette gen pass untuk quality
```

#### 6.11.4 Parameters

| Param | Type | Range | Default | Description |
|-------|------|-------|---------|-------------|
| `fps` | u8 | 1-30 | 8 | Playback FPS |
| `output_format` | enum | gif/mp4/webm | gif | Preview format |
| `loop` | bool | - | true | Loop GIF (mp4/webm always loop di players) |
| `upscale` | u8 | 1-4 | 1 | Integer upscale (nearest neighbor) |
| `frame_size` | u16 | - | auto | If sheet input, frame size (auto from sibling JSON) |

#### 6.11.5 Performance target

- 32 frames @ 256×256 → ~1MB GIF: <2 detik
- 60 frames @ 512×512 → MP4: <3 detik

---

## 7. Interface contracts

### 7.1 CLI (binary: `pixiekit-cli`)

#### 7.1.1 Global flags

```
--config <path>     Load preset JSON (override CLI args partially)
--json              Output structured JSON (for AI / scripting)
--quiet             Suppress progress, errors only
--verbose           Debug logging
--help              Show subcommand help
--version           Show version
```

#### 7.1.2 Subcommand: `bg-remove`

```
pixiekit-cli bg-remove --input <PATH> --output <PATH> [OPTIONS]

Required:
  --input <PATH>          Input file or folder
  --output <PATH>         Output folder (created if missing)

Options:
  --target-color <HEX>    Default: #00FF00
  --fuzz <FLOAT>          0.0-1.0, default: 0.35
  --despill               Default: true (use --no-despill to disable)
  --erode <N>             0-5, default: 1
  --format <png|webp>     Default: png
  --webp-quality <N>      0-100, default: 90
  --recursive             Process subfolders, default: false
  --overwrite             Overwrite existing output files, default: false
  --dry-run               Print plan, don't write
```

JSON output (`--json`):
```json
{
  "tool": "bg-remove",
  "processed": 32,
  "skipped": 0,
  "failed": 0,
  "duration_ms": 8500,
  "files": [
    {"input": "wave_01.png", "output": "wave_01.png", "status": "ok", "duration_ms": 234},
    ...
  ]
}
```

#### 7.1.3 Subcommand: `vectorize`

```
pixiekit-cli vectorize --input <PATH> --output <PATH> [OPTIONS]

Required:
  --input <PATH>             Input file or folder
  --output <PATH>             Output folder

Options:
  --mode <color|binary>       Default: color
  --filter-speckle <N>        Default: 4
  --color-precision <N>       Default: 6
  --layer-difference <N>      Default: 16
  --corner-threshold <N>      Default: 60
  --length-threshold <FLOAT>  Default: 4.0
  --splice-threshold <N>      Default: 45
  --path-precision <N>        Default: 8
  --smooth <N>                0-10 simple slider (overrides corner/length/splice)
```

#### 7.1.4 Subcommand: `video-to-sprite`

```
pixiekit-cli video-to-sprite --input <PATH> --output <PATH> [OPTIONS]

Required:
  --input <PATH>          Video file or folder of videos
  --output <PATH>         Output folder

Options:
  --fps <N>               Default: 8
  --size <N>              Default: 256
  --format <png|webp>     Default: webp
  --webp-quality <N>      Default: 90
  --chroma-key            Enable BG removal pasca-extract
  --chroma-target <HEX>   Default: #00FF00
  --chroma-fuzz <FLOAT>   Default: 0.35
  --chroma-despill        Default: true if --chroma-key
  --chroma-erode <N>      Default: 1 if --chroma-key
```

#### 7.1.5 Subcommand: `preset`

```
pixiekit-cli preset save <NAME> --tool <bg-remove|vectorize|video-to-sprite> [args...]
pixiekit-cli preset list
pixiekit-cli preset show <NAME>
pixiekit-cli preset delete <NAME>
```

Preset disimpan di `~/.config/pixiekit/presets/<name>.json`.

#### 7.1.6 Subcommand: `atlas-pack` (Phase 7)

```
pixiekit-cli atlas-pack --input <PATH> --output <PATH> [OPTIONS]

Required:
  --input <PATH>          Folder berisi PNG sprites
  --output <PATH>         Output folder (atlas.png + atlas.json ditulis ke sini)

Options:
  --name <STR>            Atlas basename, default: atlas
  --max-size <N>          256-8192, default: 2048
  --padding <N>           0-16, default: 2
  --extrude <N>           0-4, default: 1
  --power-of-two          Default: true (--no-power-of-two to disable)
  --trim                  Default: true (--no-trim to disable)
  --format <png|webp>     Default: png
  --webp-quality <N>      0-100, default: 90
  --recursive             Process subfolders, default: false
```

#### 7.1.7 Subcommand: `optimize` (Phase 8)

```
pixiekit-cli optimize --input <PATH> --output <PATH> [OPTIONS]

Options:
  --target-format <png|webp|keep>     Default: webp
  --quality <N>                        0-100, default: 90
  --lossless                           Default: false
  --strip-metadata                     Default: true
  --optimization-level <N>             0-6, default: 3
  --recursive                          Default: false
```

#### 7.1.8 Subcommand: `scale` (Phase 8)

```
pixiekit-cli scale --input <PATH> --output <PATH> [OPTIONS]

Options:
  --base-scale <FLOAT>                 Default: 4.0
  --scales <FLOAT,...>                 Default: 1.0,1.5,2.0,3.0
  --naming <flutter|suffix|nested>     Default: flutter
  --filter <lanczos|bilinear|nearest>  Default: lanczos
  --recursive                          Default: false
```

#### 7.1.9 Subcommand: `audio` (Phase 9)

```
pixiekit-cli audio --input <PATH> --output <PATH> [OPTIONS]

Options:
  --target-format <ogg|opus|mp3|wav>   Default: ogg
  --target-lufs <FLOAT>                 Default: -16.0
  --normalize                           Default: true (--no-normalize to disable)
  --trim-silence                        Default: true (--no-trim-silence to disable)
  --silence-threshold-db <FLOAT>        Default: -50.0
  --sample-rate <N>                     Default: 44100
  --channels <mono|stereo|keep>         Default: keep
  --bitrate-kbps <N>                    Default: 128
  --recursive                           Default: false
```

#### 7.1.10 Subcommand: `trim-pad` (Phase 10)

```
pixiekit-cli trim-pad --input <PATH> --output <PATH> [OPTIONS]

Options:
  --alpha-threshold <N>     0-255, default: 1
  --padding <N>             0-256, default: 0
  --keep-square             Default: false
  --bg-color <HEX>          Optional, treats this color as transparent
  --bg-tolerance <FLOAT>    0.0-1.0, default: 0.05 (only if --bg-color set)
  --recursive               Default: false
```

#### 7.1.11 Subcommand: `svg-optimize` (Phase 10)

```
pixiekit-cli svg-optimize --input <PATH> --output <PATH> [OPTIONS]

Options:
  --precision <N>          0-10, default: 3
  --remove-metadata        Default: true
  --remove-hidden          Default: true
  --merge-paths            Default: true
  --pretty                 Default: false
  --recursive              Default: false
```

#### 7.1.12 Subcommand: `nine-slice` (Phase 11)

```
pixiekit-cli nine-slice --input <PATH> --output <PATH> \
                        --top <N> --right <N> --bottom <N> --left <N> [OPTIONS]

Required:
  --input <PATH>                   Single PNG file
  --output <PATH>                  Output folder
  --top <N>                        Top inset (px)
  --right <N>                      Right inset (px)
  --bottom <N>                     Bottom inset (px)
  --left <N>                       Left inset (px)

Options:
  --output-mode <split|metadata>   Default: metadata
```

#### 7.1.13 Subcommand: `anim-preview` (Phase 11)

```
pixiekit-cli anim-preview --input <PATH> --output <PATH> [OPTIONS]

Options:
  --fps <N>                        1-30, default: 8
  --output-format <gif|mp4|webm>   Default: gif
  --loop                           Default: true (--no-loop to disable)
  --upscale <N>                    1-4, default: 1
  --frame-size <N>                 Auto-detect from sibling JSON if sheet input
```

#### 7.1.14 Exit codes

| Code | Meaning |
|------|---------|
| 0 | All files processed successfully |
| 1 | At least one file failed (others may have succeeded) |
| 2 | Invalid arguments / missing required flag |
| 3 | Input path not found / not readable |
| 4 | Output path not writable |
| 5 | Missing system dependency (ffmpeg not found, etc.) |

### 7.2 GUI (Tauri app: `pixiekit`)

#### 7.2.1 Window structure

- Main window: 1200×800 default, resizable, min 800×600
- Single-window app, **tab-based** untuk 3 tools (no multi-window)
- Native menu bar (File / Edit / View / Window / Help)

#### 7.2.2 Layout per tab

```
┌─ Tool tabs ──────────────────────────────────────┐
│ [BG Remove] [Vectorize] [Video → Sprite]         │
├──────────────────────────────────────────────────┤
│ Input:   [path___________] [Browse] [Drag here] │
│ Output:  [path___________] [Browse] [Drag here] │
├──────────────────────────────────────────────────┤
│ ┌─ Settings panel ─────────────────────────────┐ │
│ │ (slider/checkbox/dropdown per parameter)     │ │
│ │ [Save preset...] [Load preset ▼]            │ │
│ └──────────────────────────────────────────────┘ │
├──────────────────────────────────────────────────┤
│ ┌─ Preview ────────────────────────────────────┐ │
│ │  [Before]              [After]               │ │
│ │  (canvas)              (canvas, checker bg)  │ │
│ │                                              │ │
│ │  Pick image to preview: [filename.png ▼]    │ │
│ └──────────────────────────────────────────────┘ │
├──────────────────────────────────────────────────┤
│ Files in input: 12 found                        │
│ ┌──────────────────────────────────────────────┐ │
│ │ ☑ wave_01.png    1024×1024              ─▷  │ │
│ │ ☑ wave_02.png    1024×1024              ─▷  │ │
│ │ ...                                         │ │
│ └──────────────────────────────────────────────┘ │
│            [▶ Process All]   [Stop]              │
├──────────────────────────────────────────────────┤
│ Log:                                             │
│  ✓ wave_01.png → /output/wave_01.png  (234ms)   │
│  ✓ wave_02.png → /output/wave_02.png  (210ms)   │
└──────────────────────────────────────────────────┘
```

#### 7.2.3 Tauri commands (frontend ↔ backend bridge)

```rust
#[tauri::command] async fn pick_folder() -> Result<String>
#[tauri::command] async fn list_input_files(path: String, exts: Vec<String>) -> Result<Vec<FileInfo>>
#[tauri::command] async fn preview_bg_remove(path: String, opts: BgRemoveOpts) -> Result<Vec<u8>>  // returns processed PNG bytes
#[tauri::command] async fn run_bg_remove(opts: BgRemoveBatchOpts, on_progress: Channel) -> Result<RunReport>
#[tauri::command] async fn preview_vectorize(path: String, opts: VectorizeOpts) -> Result<String>  // returns SVG string
#[tauri::command] async fn run_vectorize(opts: VectorizeBatchOpts, on_progress: Channel) -> Result<RunReport>
#[tauri::command] async fn run_video_to_sprite(opts: VideoToSpriteBatchOpts, on_progress: Channel) -> Result<RunReport>
#[tauri::command] async fn list_presets() -> Result<Vec<String>>
#[tauri::command] async fn save_preset(name: String, content: serde_json::Value) -> Result<()>
#[tauri::command] async fn load_preset(name: String) -> Result<serde_json::Value>
```

### 7.3 MCP server (binary: `pixiekit-mcp`)

#### 7.3.1 Transport

- **stdio** (standard MCP transport untuk Claude Code)
- Bukan HTTP/SSE — keep simple

#### 7.3.2 Tool definitions

```json
[
  {
    "name": "bg_remove",
    "description": "Remove green/blue screen background from images. Batch process folder.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "input": {"type": "string", "description": "Path to image file or folder"},
        "output": {"type": "string", "description": "Output folder path"},
        "target_color": {"type": "string", "default": "#00FF00", "pattern": "^#[0-9a-fA-F]{6}$"},
        "fuzz": {"type": "number", "minimum": 0, "maximum": 1, "default": 0.35},
        "despill": {"type": "boolean", "default": true},
        "erode": {"type": "integer", "minimum": 0, "maximum": 5, "default": 1},
        "format": {"type": "string", "enum": ["png", "webp"], "default": "png"}
      },
      "required": ["input", "output"]
    }
  },
  {
    "name": "vectorize",
    "description": "Convert raster image (PNG/JPG/WebP) to SVG vector path.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "input": {"type": "string"},
        "output": {"type": "string"},
        "mode": {"type": "string", "enum": ["color", "binary"], "default": "color"},
        "smooth": {"type": "integer", "minimum": 0, "maximum": 10, "default": 4}
      },
      "required": ["input", "output"]
    }
  },
  {
    "name": "video_to_sprite",
    "description": "Extract frames from video and stitch into horizontal sprite sheet for game engines.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "input": {"type": "string"},
        "output": {"type": "string"},
        "fps": {"type": "integer", "minimum": 1, "maximum": 30, "default": 8},
        "size": {"type": "integer", "minimum": 64, "maximum": 1024, "default": 256},
        "format": {"type": "string", "enum": ["png", "webp"], "default": "webp"},
        "chroma_key": {"type": "boolean", "default": false}
      },
      "required": ["input", "output"]
    }
  },
  {
    "name": "list_presets",
    "description": "List saved processing presets.",
    "inputSchema": {"type": "object", "properties": {}}
  },
  {
    "name": "atlas_pack",
    "description": "Pack a folder of PNG sprites into a texture atlas with Flame-compatible JSON metadata. Reduces draw calls in game runtime.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "input": {"type": "string", "description": "Folder of PNG sprites"},
        "output": {"type": "string", "description": "Output folder for atlas + JSON"},
        "name": {"type": "string", "default": "atlas"},
        "max_size": {"type": "integer", "minimum": 256, "maximum": 8192, "default": 2048},
        "padding": {"type": "integer", "minimum": 0, "maximum": 16, "default": 2},
        "extrude": {"type": "integer", "minimum": 0, "maximum": 4, "default": 1},
        "power_of_two": {"type": "boolean", "default": true},
        "trim": {"type": "boolean", "default": true},
        "format": {"type": "string", "enum": ["png", "webp"], "default": "png"}
      },
      "required": ["input", "output"]
    }
  },
  {
    "name": "optimize_image",
    "description": "Optimize PNG/JPG/WebP images: quantization, format conversion, metadata stripping.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "input": {"type": "string"},
        "output": {"type": "string"},
        "target_format": {"type": "string", "enum": ["png", "webp", "keep"], "default": "webp"},
        "quality": {"type": "integer", "minimum": 0, "maximum": 100, "default": 90},
        "lossless": {"type": "boolean", "default": false},
        "strip_metadata": {"type": "boolean", "default": true},
        "optimization_level": {"type": "integer", "minimum": 0, "maximum": 6, "default": 3}
      },
      "required": ["input", "output"]
    }
  },
  {
    "name": "scale_image",
    "description": "Generate multi-resolution variants from source image (Flutter @1x/@2x/@3x or iOS-style suffix).",
    "inputSchema": {
      "type": "object",
      "properties": {
        "input": {"type": "string"},
        "output": {"type": "string"},
        "base_scale": {"type": "number", "minimum": 1.0, "maximum": 8.0, "default": 4.0},
        "target_scales": {"type": "array", "items": {"type": "number"}, "default": [1.0, 1.5, 2.0, 3.0]},
        "naming": {"type": "string", "enum": ["flutter", "suffix", "nested"], "default": "flutter"},
        "filter": {"type": "string", "enum": ["lanczos", "bilinear", "nearest"], "default": "lanczos"}
      },
      "required": ["input", "output"]
    }
  },
  {
    "name": "audio_process",
    "description": "Process audio files: LUFS normalize, trim silence, format convert. Wraps ffmpeg.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "input": {"type": "string"},
        "output": {"type": "string"},
        "target_format": {"type": "string", "enum": ["ogg", "opus", "mp3", "wav"], "default": "ogg"},
        "target_lufs": {"type": "number", "minimum": -30, "maximum": 0, "default": -16.0},
        "normalize": {"type": "boolean", "default": true},
        "trim_silence": {"type": "boolean", "default": true},
        "silence_threshold_db": {"type": "number", "default": -50.0},
        "sample_rate": {"type": "integer", "default": 44100},
        "channels": {"type": "string", "enum": ["mono", "stereo", "keep"], "default": "keep"},
        "bitrate_kbps": {"type": "integer", "default": 128}
      },
      "required": ["input", "output"]
    }
  },
  {
    "name": "trim_pad",
    "description": "Auto-crop transparent borders and add uniform padding. Useful pre-step for atlas_pack.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "input": {"type": "string"},
        "output": {"type": "string"},
        "alpha_threshold": {"type": "integer", "minimum": 0, "maximum": 255, "default": 1},
        "padding": {"type": "integer", "minimum": 0, "maximum": 256, "default": 0},
        "keep_square": {"type": "boolean", "default": false},
        "bg_color": {"type": "string", "pattern": "^#[0-9a-fA-F]{6}$"},
        "bg_tolerance": {"type": "number", "minimum": 0, "maximum": 1, "default": 0.05}
      },
      "required": ["input", "output"]
    }
  },
  {
    "name": "svg_optimize",
    "description": "Minify SVG files: round coordinates, strip metadata, remove hidden elements.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "input": {"type": "string"},
        "output": {"type": "string"},
        "precision": {"type": "integer", "minimum": 0, "maximum": 10, "default": 3},
        "remove_metadata": {"type": "boolean", "default": true},
        "remove_hidden": {"type": "boolean", "default": true},
        "merge_paths": {"type": "boolean", "default": true},
        "pretty": {"type": "boolean", "default": false}
      },
      "required": ["input", "output"]
    }
  },
  {
    "name": "nine_slice",
    "description": "Generate 9-slice metadata for UI buttons/panels (Flame NineTileBoxComponent compatible). Split or metadata mode.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "input": {"type": "string", "description": "Single PNG file"},
        "output": {"type": "string"},
        "top": {"type": "integer", "minimum": 0},
        "right": {"type": "integer", "minimum": 0},
        "bottom": {"type": "integer", "minimum": 0},
        "left": {"type": "integer", "minimum": 0},
        "output_mode": {"type": "string", "enum": ["split", "metadata"], "default": "metadata"}
      },
      "required": ["input", "output", "top", "right", "bottom", "left"]
    }
  },
  {
    "name": "anim_preview",
    "description": "Generate GIF/MP4/WebM preview from sprite sheet or folder of frame PNGs. Uses ffmpeg.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "input": {"type": "string", "description": "Sprite sheet PNG or folder of frame PNGs"},
        "output": {"type": "string"},
        "fps": {"type": "integer", "minimum": 1, "maximum": 30, "default": 8},
        "output_format": {"type": "string", "enum": ["gif", "mp4", "webm"], "default": "gif"},
        "loop": {"type": "boolean", "default": true},
        "upscale": {"type": "integer", "minimum": 1, "maximum": 4, "default": 1},
        "frame_size": {"type": "integer", "description": "Auto-detect from sibling JSON if omitted"}
      },
      "required": ["input", "output"]
    }
  }
]
```

#### 7.3.3 Tool response format

```json
{
  "content": [
    {
      "type": "text",
      "text": "Processed 32 files in 8.5s. Output: /Users/.../cleaned/"
    }
  ],
  "structuredContent": {
    "processed": 32,
    "failed": 0,
    "duration_ms": 8500,
    "output_dir": "/Users/.../cleaned/"
  }
}
```

#### 7.3.4 Registration

User registers MCP server di Claude Code:
```bash
claude mcp add pixiekit -- /Users/mochammadlutfi/domdom/pixiekit/target/release/pixiekit-mcp
```

---

## 8. Tech stack

### 8.1 Runtime

- **Rust** stable (1.80+)
- **Tauri** 2.x — desktop app framework
- **ffmpeg** — system dependency (already installed via Homebrew)

### 8.2 Rust crates

| Crate | Version | Purpose | Used by |
|-------|--------:|---------|---------|
| `image` | 0.25 | PNG/JPG/WebP read+write | core |
| `imageproc` | 0.25 | Morphology (erode), filters | core |
| `vtracer` | 0.6 | Raster→SVG | core |
| `rayon` | 1.10 | Parallel iteration | core |
| `serde` | 1 | (De)serialization | all |
| `serde_json` | 1 | JSON I/O | all |
| `anyhow` | 1 | Error context | all |
| `thiserror` | 1 | Custom error types | core |
| `clap` | 4 | CLI parser (derive) | cli |
| `indicatif` | 0.17 | Progress bars in CLI | cli |
| `tauri` | 2 | Desktop framework | gui |
| `tauri-build` | 2 | Build script | gui (build-dep) |
| `tokio` | 1 | Async runtime | gui, mcp |
| `tracing` | 0.1 | Structured logging | all |
| `tracing-subscriber` | 0.3 | Log subscriber | all |
| `mcp-sdk` | TBD | MCP server impl (or hand-roll) | mcp |
| `walkdir` | 2 | Recursive folder traversal | core |
| `tempfile` | 3 | Temp dirs for ffmpeg frames | core |
| `directories` | 5 | XDG paths (preset storage) | core |

### 8.3 Why these choices

- **`image` over ImageMagick CLI:** native Rust, no external dependency, faster (no fork+exec overhead), better error handling
- **`vtracer` direct crate import** vs subprocess: zero overhead, type-safe params, no shelling out
- **`ffmpeg` as subprocess** vs `ffmpeg-next` Rust bindings: bindings are heavyweight (need libavcodec etc. headers), subprocess simpler & sufficient for our use
- **`rayon` over manual threading:** trivially parallel batch processing
- **Tauri over Electron:** ~10× smaller bundle, native performance, modern Rust ecosystem
- **stdio MCP** over HTTP MCP: standard for Claude Code, no port management

---

## 9. File system conventions

### 9.1 Config / state

```
~/.config/pixiekit/
├── config.toml                  # Global settings (default paths, last-used opts)
├── presets/
│   ├── domdom-bg-clean.json
│   ├── domdom-sprite-8fps.json
│   └── ...
└── recent.json                  # Last 10 input/output paths (for GUI dropdown)
```

`config.toml` structure:
```toml
[general]
ffmpeg_path = "/opt/homebrew/bin/ffmpeg"  # auto-detected, override if needed
default_output_format = "webp"
default_webp_quality = 90

[gui]
last_window_size = [1200, 800]
last_active_tab = "bg-remove"
```

Preset JSON structure:
```json
{
  "name": "domdom-bg-clean",
  "tool": "bg-remove",
  "version": 1,
  "options": {
    "target_color": "#00FF00",
    "fuzz": 0.35,
    "despill": true,
    "erode": 1,
    "format": "png"
  }
}
```

### 9.2 Logs

- GUI: in-app log panel (cleared on app close)
- CLI: stderr (default), `--verbose` opens debug-level
- MCP: stderr only (stdout is reserved for MCP protocol)

Optional persistent log: `~/Library/Logs/pixiekit/pixiekit.log` (Mac convention) — opt-in via env `PIXIEKIT_LOG_FILE=1`.

### 9.3 Temp files

ffmpeg frame extraction → `tempfile::TempDir` (auto-cleanup). Path: `/tmp/pixiekit-<uuid>/`.

---

## 10. Build & distribution

### 10.1 Development workflow

```bash
# Clone (or work in-place)
cd ~/domdom/pixiekit

# Run CLI in dev
cargo run --bin pixiekit-cli -- bg-remove --input ./test/raw --output ./test/clean

# Run GUI in dev (with hot reload)
cargo tauri dev --binary pixiekit

# Run MCP in dev
cargo run --bin pixiekit-mcp

# Test
cargo test --workspace

# Lint
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

### 10.2 Release build

```bash
cargo build --release --workspace

# GUI bundle (.app for Mac)
cargo tauri build --binary pixiekit

# Output:
# target/release/pixiekit-cli           (~5-10 MB)
# target/release/pixiekit-mcp           (~5-10 MB)
# target/release/bundle/macos/Pixiekit.app (~20-30 MB with webview)
```

### 10.3 Installation untuk user

**Manual (user is the developer):**
```bash
cargo build --release --workspace
sudo ln -s $(pwd)/target/release/pixiekit-cli /usr/local/bin/pixiekit-cli
sudo ln -s $(pwd)/target/release/pixiekit-mcp /usr/local/bin/pixiekit-mcp
cp -r target/release/bundle/macos/Art\ Tools.app /Applications/
```

**Untuk MCP integration:**
```bash
claude mcp add pixiekit -- /usr/local/bin/pixiekit-mcp
```

### 10.4 Cross-platform targets (future)

- macOS Apple Silicon (`aarch64-apple-darwin`) — primary
- macOS Intel (`x86_64-apple-darwin`) — opt-in
- Linux x86_64 (`x86_64-unknown-linux-gnu`) — opt-in
- Windows (`x86_64-pc-windows-gnu`) — future, requires Tauri WebView2 setup

CI: GitHub Actions matrix build saat tag `v*` di-push (out of scope phase 1).

---

## 11. Phased roadmap

### Phase 1 — Foundation (target: 2-3 hari kerja)

- [ ] M1.1: Setup Cargo workspace + 4 crate skeleton (`core`, `cli`, `gui`, `mcp`)
- [ ] M1.2: `core::bg_remove` — port logic dari `clean-bg.py` ke Rust + unit tests
- [ ] M1.3: `cli` binary subcommand `bg-remove` — single + batch
- [ ] M1.4: Verify parity dengan output `clean-bg.py` existing (5 sprite sheet test)
- [ ] M1.5: Dokumentasi: README install + run basic, CLI.md untuk `bg-remove`

**Deliverable Phase 1:** AI bisa pakai `pixiekit-cli bg-remove` menggantikan `clean-bg.py`.

### Phase 2 — GUI (target: 1-2 hari)

- [ ] M2.1: Tauri app shell (window, tab navigation, native menu)
- [ ] M2.2: Folder picker via `rfd` crate (Tauri command)
- [ ] M2.3: BG Remove tab UI (settings panel, file list, preview canvas)
- [ ] M2.4: Live preview (Tauri command `preview_bg_remove` returns PNG bytes)
- [ ] M2.5: Batch process dengan progress streaming via Tauri Channel

**Deliverable Phase 2:** User bisa drag folder ke GUI → tweak → preview → batch.

### Phase 3 — Video → Sprite (target: 1-2 hari)

- [ ] M3.1: `core::video_to_sprite` — ffmpeg subprocess wrapper + frame stitch
- [ ] M3.2: Optional chroma key integration (reuse `core::bg_remove`)
- [ ] M3.3: Metadata JSON sibling output
- [ ] M3.4: CLI subcommand `video-to-sprite`
- [ ] M3.5: GUI tab "Video → Sprite"

**Deliverable Phase 3:** Replace `extract-sprites-smooth.sh` + `stitch-sprites-smooth.sh`.

### Phase 4 — Vectorize (target: 1-2 hari)

- [ ] M4.1: `core::vectorize` — vtracer crate wrapper
- [ ] M4.2: CLI subcommand `vectorize`
- [ ] M4.3: GUI tab "Vectorize" dengan SVG live preview (render via embedded WebView)
- [ ] M4.4: "Smoothness" simple slider mapping ke advanced params

**Deliverable Phase 4:** End-to-end pipeline: AI image → BG clean → SVG trace → editor handoff.

### Phase 5 — MCP Server (target: 1 hari)

- [ ] M5.1: MCP server skeleton (stdio transport)
- [ ] M5.2: Tool registration (3 tools)
- [ ] M5.3: Test integration dengan Claude Code
- [ ] M5.4: Dokumentasi MCP.md

**Deliverable Phase 5:** Claude Code bisa panggil tools via MCP native (selain CLI subprocess).

### Phase 6 — Polish (target: 1 hari)

- [ ] M6.1: Preset save/load (CLI + GUI)
- [ ] M6.2: Recent paths cache (GUI dropdown)
- [ ] M6.3: Tauri bundle (.dmg) build script
- [ ] M6.4: Error messages humanization
- [ ] M6.5: Performance profiling + optimasi (kalau ada hot path slow)

**Deliverable Phase 6:** Production-quality v1.0.0 release.

### Phase 7 — Sprite Atlas Packer (target: 2 hari)

Goal: 1 draw call vs N draw call → boost game perf di tablet murah (kids 3-7 device target).

- [ ] M7.1: Add dep `texture_packer` (atau `crunch`) ke `core/Cargo.toml`
- [ ] M7.2: `core::atlas_pack` — bin packing + JSON metadata generation (Flame TexturePacker JSON Hash format)
- [ ] M7.3: Optional pre-trim integration (reuse logic dari Phase 10 trim_pad kalau sudah ada, atau inline minimal trim)
- [ ] M7.4: Edge extrude (1-2px replicate) untuk anti-bleed
- [ ] M7.5: CLI subcommand `atlas-pack`
- [ ] M7.6: MCP tool `atlas_pack`
- [ ] M7.7: Web API `/api/atlas-pack` + Nuxt frontend tab
- [ ] M7.8: Unit tests: packing efficiency >75%, metadata schema valid
- [ ] M7.9: Integration test dengan Domdom existing sprites — verify Flame load dari atlas

**Deliverable Phase 7:** Domdom World bisa pakai 1 atlas untuk 50+ sprites → frame rate stabil 60fps di tablet entry-level.

**Risks:** Bin packing non-deterministic kalau ada banyak sprite same-size — perlu stable sort untuk reproducibility.

### Phase 8 — Image Optimizer + Multi-Resolution Scaler (target: 2 hari)

Goal: Reduce app size + serve correct DPI ke device beragam.

- [ ] M8.1: Add deps `oxipng` + verify `image` crate WebP feature aktif
- [ ] M8.2: `core::optimize` — PNG quantize, WebP encode, metadata strip
- [ ] M8.3: `core::scale` — multi-scale Lanczos resize dengan output naming variants (Flutter / suffix / nested)
- [ ] M8.4: CLI subcommands `optimize` + `scale`
- [ ] M8.5: MCP tools `optimize_image` + `scale_image`
- [ ] M8.6: Web API + Nuxt frontend
- [ ] M8.7: Benchmark: PNG→WebP size reduction target 30-60%, optimize 100 files <30 detik

**Deliverable Phase 8:** Domdom asset bundle size turun signifikan, multi-DPI siap untuk Flutter `flutter:assets:` variant.

### Phase 9 — Audio Processor (target: 1-2 hari)

Goal: Standardize SFX & narasi pipeline (Domdom kids game = banyak audio).

- [ ] M9.1: ffmpeg subprocess wrapper di `core::audio` (reuse pattern dari `video_to_sprite`)
- [ ] M9.2: LUFS normalize via ffmpeg `loudnorm` filter (2-pass untuk akurasi)
- [ ] M9.3: Silence trim via `silenceremove` filter
- [ ] M9.4: Format convert (OGG default, OPUS, MP3, WAV)
- [ ] M9.5: CLI subcommand `audio`
- [ ] M9.6: MCP tool `audio_process`
- [ ] M9.7: Web API + Nuxt frontend (preview player widget)
- [ ] M9.8: Test: 50 SFX → konsisten -16 LUFS ±0.5

**Deliverable Phase 9:** Volume audio Domdom konsisten di semua device, no jarring loud SFX (penting untuk safety telinga anak 3-7).

### Phase 10 — Trim & Pad + SVG Optimizer (target: 1-2 hari)

Goal: Pre-step untuk Atlas Packer + post-step untuk Vectorize.

- [ ] M10.1: `core::trim_pad` — alpha bbox detection + uniform padding
- [ ] M10.2: Optional bg_color tolerance mode (untuk image yang belum punya alpha)
- [ ] M10.3: `core::svg_optimize` — `usvg` parse + minify + path precision rounding
- [ ] M10.4: Path merging untuk SVG dengan style sama (optional, behind flag)
- [ ] M10.5: CLI subcommands `trim-pad` + `svg-optimize`
- [ ] M10.6: MCP tools `trim_pad` + `svg_optimize`
- [ ] M10.7: Web API + Nuxt frontend
- [ ] M10.8: Test: SVG size reduction target 30-60%, trim accuracy ±0px on test fixtures

**Deliverable Phase 10:** Pipeline lengkap: AI image → BG remove → trim-pad → atlas pack (raster path) atau → vectorize → svg-optimize (vector path).

### Phase 11 — 9-Slice Slicer + Animation Preview (target: 1-2 hari)

Goal: UI workflow (Flame NineTileBox) + asset review tool.

- [ ] M11.1: `core::nine_slice` — split mode (9 PNG output) + metadata mode (JSON sibling)
- [ ] M11.2: Validate insets (sum top+bottom < height, left+right < width)
- [ ] M11.3: `core::anim_preview` — sprite sheet split + ffmpeg compose to GIF/MP4/WebM
- [ ] M11.4: Auto-detect frame_size dari sibling JSON (kalau ada hasil video-to-sprite)
- [ ] M11.5: GIF palette generation pass untuk quality (ffmpeg palettegen + paletteuse)
- [ ] M11.6: CLI subcommands `nine-slice` + `anim-preview`
- [ ] M11.7: MCP tools `nine_slice` + `anim_preview`
- [ ] M11.8: Web API + Nuxt frontend (with embedded video/img preview)

**Deliverable Phase 11:** Designer Domdom bisa generate UI button assets + preview animasi sprite sheet tanpa perlu run game runtime.

### Phase 12 — Tauri Desktop Bundle ✅ done

Goal: Native desktop app sebagai alternatif Nuxt SaaS frontend untuk user yang offline-first / tidak mau internet-dependent. Reuse `core` crate sepenuhnya — Tauri jadi GUI shell ke-2 (CLI + Web + Desktop).

**Lokasi crate**: `apps/web/src-tauri/` (bukan `crates/gui/` seperti spec awal). Alasan: reuse Nuxt 4 frontend yang sama persis dengan SaaS — Tauri webview load `apps/web/.output/public`. Crate name `pixiekit-gui`, binary name `pixiekit-gui`.

- [x] M12.1: Tauri 2.x app shell — window 1200×800 (min 800×600), native menu File / Edit / View / Window / Help via `MenuBuilder`/`SubmenuBuilder` predefined items
- [x] M12.2: Folder/file picker via `tauri-plugin-dialog` (cross-platform native dialog)
- [x] M12.3: Tauri commands wiring per tool (11 `run_*` commands, semua di `apps/web/src-tauri/src/lib.rs`):
  - `run_bg_remove`, `run_vectorize`, `run_video_to_sprite` (Phase 1-3)
  - `run_atlas_pack`, `run_optimize`, `run_scale` (Phase 7-8)
  - `run_audio`, `run_trim_pad`, `run_svg_optimize` (Phase 9-10)
  - `run_nine_slice`, `run_anim_preview` (Phase 11)
- [x] M12.4: Live preview commands — `preview_bg_remove`, `preview_vectorize`, `preview_trim_pad` (PNG bytes / SVG string base64)
- [x] M12.5: Batch progress streaming via `tauri::ipc::Channel<ProgressPayload>` di semua `run_*` commands
- [x] M12.6: Preset CRUD commands — `list_presets`, `get_preset`, `save_preset`, `delete_preset` (reuse `core::preset`)
- [x] M12.7: Recent paths cache — `core::recent` module (`~/.config/pixiekit/recent.json`, max 10 entries per kind), exposed via `list_recent_paths` / `add_recent_path` / `clear_recent_paths`
- [x] M12.8: Audio preview — frontend embed `<audio>` di webview (no extra Tauri command needed; output di filesystem yang user pilih)
- [x] M12.9: Animation preview — frontend embed GIF/MP4 via `<img>`/`<video>` tag
- [x] M12.10: Bundle build script — `scripts/build-desktop.sh` wraps `pnpm tauri build` (auto-detect target OS, optional `--bundles dmg|appimage,deb|msi`)
- [ ] M12.11: Code signing + notarization (Mac) — **deferred**, butuh Apple Developer account ($99/yr); README dokumentasikan Gatekeeper override workaround untuk first run
- [x] M12.12: README install instructions — section "Desktop app (Phase 12 — Tauri bundle)" di `README.md` (prereq, dev mode, release bundle, signing note, recent paths cache). Screenshot/demo GIF: TODO setelah ada visual asset.

**Deliverable Phase 12:** Native `Pixiekit.app` (~30 MB dengan webview) installable di `/Applications/`. Fully offline, no localhost server. User bisa pilih: Web SaaS (Phase 5b) atau Desktop bundle (Phase 12).

**Risks:**
- Tauri v2 ekosistem masih early — beberapa plugin (e.g. `rfd`, dialog) butuh adapt jika API berubah
- Webview render preview slow untuk image 4K+ (mitigation: downscale before send to frontend, max 1024px preview)
- Bundle size +20MB karena webview — acceptable untuk desktop app
- Code signing fee Apple Developer ($99/year) — kalau tidak signed, user perlu manual Gatekeeper override saat first run

**Decision criteria untuk start:**
- Phase 7-11 selesai (Tauri commands wiring jadi banyak — tunda sampai semua tool stable)
- ATAU user request offline-first urgent (skip Phase 7-11 yang belum prioritas)

### Total estimasi (revised)

| Range | Scope | Cumulative |
|-------|-------|-----------|
| Phase 1-6 | Foundation + 3 core tools + MCP + SaaS + Polish | ✅ done |
| Phase 7-11 | 8 tool tambahan untuk completing asset pipeline | ~8-10 hari kerja |
| Phase 12 | Tauri desktop bundle (offline-first GUI) | ~3-4 hari kerja |

Phase 7-12 bisa di-prioritize berdasarkan kebutuhan Domdom production:
- **Tier 1 (impact tertinggi)**: Phase 7 (Atlas Packer) — game perf
- **Tier 2 (shipping ready)**: Phase 8 (Optimizer + Scaler) — app size
- **Tier 3 (quality of life)**: Phase 9, 10, 11
- **Tier 4 (alt frontend)**: Phase 12 — hanya kalau ada demand offline-first user

Phase 7-11 saling independen — bisa dikerjakan dalam urutan apapun (semua extend `core` dengan modul baru, no shared state changes). Phase 12 sebaiknya **last** karena perlu wiring semua tool ke Tauri commands.

---

## 12. Open questions

| ID | Question | Owner | Resolution target |
|----|----------|:-----:|:-----------------:|
| Q1 | MCP SDK Rust mana yang paling mature? Hand-roll stdio JSON-RPC vs pakai crate? | Tech | Phase 5 start |
| Q2 | Tauri v2 vs v1 — v2 stable, tapi ekosistem masih early. Cek breaking changes tunggu rilis. | Tech | Phase 2 start |
| Q3 | vtracer parameter "Smoothness" simple slider — formula mapping ke advanced params apa? | Product | Phase 4 design |
| Q4 | Preview canvas size limit di GUI — kalau image 4K, render ke canvas berapa MB RAM? | Tech | Phase 2 implementation |
| Q5 | Cross-compile ke Windows worth it untuk solo project? Atau Mac-only acceptable? | Product | Future |
| Q6 | Auto-update mechanism — pakai Tauri updater atau manual download? | Product | Phase 6 |
| Q7 | Asset distribution — Tauri bundle disimpan di Github Release atau Dropbox? | Ops | Phase 6 |
| Q8 | Atlas Packer: pakai `texture_packer` atau `crunch`? Benchmark efficiency dulu. | Tech | Phase 7 start |
| Q9 | Multi-Resolution Scaler default `base_scale=4.0` cocok atau perlu auto-detect dari image dimension + naming convention? | Product | Phase 8 design |
| Q10 | Audio: `loudnorm` 1-pass cukup atau perlu 2-pass untuk akurasi -16 LUFS? Tradeoff speed vs precision. | Tech | Phase 9 start |
| Q11 | SVG Optimizer: pakai `usvg` (Rust pure) atau invoke SVGO (Node.js subprocess)? | Tech | Phase 10 start |
| Q12 | Atlas JSON format: TexturePacker JSON Hash (Flame default) atau custom Pixiekit format? | Product | Phase 7 design |
| Q13 | Tauri code signing — beli Apple Developer account ($99/yr) atau ship unsigned dengan README workaround? | Ops | Phase 12 start |
| Q14 | Tauri desktop perlu support semua 11 tools atau cukup core 3 tools (BG/Vectorize/Video)? Tradeoff scope vs effort. | Product | Phase 12 design |

---

## 13. Risks & mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|:----------:|:------:|------------|
| vtracer parameter UX terlalu kompleks untuk user awam | Medium | Medium | "Smoothness" simple slider + advanced toggle |
| ffmpeg subprocess fail di edge cases (corrupt video, codec exotic) | Low | High | Validate input via `ffprobe` first, clear error messages |
| Tauri webview render preview slow untuk image 4K+ | Medium | Medium | Downscale before send to frontend (max 1024px preview) |
| MCP server crash mid-batch — partial state | Low | Medium | Idempotent file write (atomic rename), report partial success |
| `image` crate WebP encoder lebih lambat dari `cwebp` CLI | Medium | Low | Benchmark; fallback shell-out kalau >2× slower |
| User input path dengan space / unicode rusak ffmpeg invocation | Medium | High | Quote args properly; test dengan path bizarre |
| Tauri v2 breaking changes pre-stable | Low | High | Lock to specific minor version, jangan pakai bleeding edge |
| Atlas Packer: bin packing fail kalau sprite > max_size | Low | Medium | Validate input dimensions sebelum pack, error message jelas dengan saran resize/multi-atlas |
| Image Optimizer: WebP encode lebih lambat dari PNG → user surprise | Medium | Low | Dokumentasi default behavior + optional `--quick` mode (skip metadata strip + lower opt level) |
| Audio LUFS normalize tidak berhasil di file pendek (<3s SFX) | Medium | Medium | Detect duration → fallback ke peak normalize untuk file pendek |
| SVG Optimizer rusak path complex (kids drawing dengan gradient) | Low | Medium | Test case Domdom-specific SVG sebelum apply ke production assets |
| 9-Slice insets invalid (sum > dimension) | Low | Low | Validate di CLI parse + return code 2 (invalid args) |
| Animation Preview GIF size besar untuk 60 frames @ 512px | Medium | Low | Default upscale=1, dokumentasi recommend MP4 untuk frame banyak |
| Tauri 2.x ecosystem masih early-stable, breaking changes possible | Medium | High | Lock `tauri = "=2.x.y"` exact version, monitor changelog before upgrade |
| Tauri webview rendering inconsistent across Mac/Linux/Windows | Medium | Medium | Test di minimal 2 platform sebelum release, gunakan CSS yang well-supported |
| Tauri bundle size besar (+webview) untuk Linux/Windows kalau butuh download WebView2 | Medium | Low | Use `tauri.bundle.windows.webviewInstallMode = "downloadBootstrapper"` (smaller bundle, runtime install) |

---

## 14. Appendix

### 14.1 Glossary

- **Chroma key** — teknik replace warna (biasanya green/blue) dengan transparency
- **Despill** — algoritma untuk hilangkan green spill (warna hijau pantulan) di edge character
- **Erode** — morphology operation yang shrink alpha mask, hilangkan jagged edge
- **Sprite sheet** — single image yang berisi multiple animation frames secara horizontal/grid
- **Lanczos** — high-quality image resampling algorithm (untuk downscale)
- **Vector tracing** — convert pixel art ke geometric path (Bezier curves)
- **MCP** — Model Context Protocol, standard untuk AI agent tool integration
- **Tauri** — Rust desktop app framework (alternatif Electron, lebih ringan)

### 14.2 Reference: existing scripts yang akan di-replace

| Existing script | Replaced by |
|-----------------|-------------|
| `~/flutter/ulin/pixiekit/rive/clean-bg.py` | `pixiekit-cli bg-remove` |
| `~/flutter/ulin/pixiekit/rive/clean-bg.sh` | `pixiekit-cli bg-remove` |
| `~/flutter/ulin/pixiekit/rive/clean-bg.html` | GUI BG Remove tab |
| `~/flutter/ulin/pixiekit/domdom-sprites/extract-sprites-smooth.sh` | `pixiekit-cli video-to-sprite` |
| `~/flutter/ulin/pixiekit/domdom-sprites/stitch-sprites-smooth.sh` | (idem — single subcommand handles full pipeline) |

Setelah Phase 3 selesai, script lama di-archive ke `flutter/ulin/pixiekit/_legacy/`, tidak dihapus dulu (rollback safety).

### 14.3 Reference: Dom Dom World asset spec relevan

- Sprite frame size: 256×256 (per `DomdomMotion.frameSize` di Flutter app)
- Sprite sheet format: WebP q=90, alpha lossless
- Chroma key BG: `#00FF00` solid (per `docs/05-assets/sprite-animation-prompts-video.md` line 129)
- Target FPS: 8fps action, 6fps slow loop (sleep)
- SVG output: digunakan untuk Glaxnimate vector animation (Lottie export path)

### 14.4 vtracer parameter cheat sheet

| Slider position | corner_threshold | length_threshold | splice_threshold | Visual effect |
|:---------------:|:----------------:|:----------------:|:----------------:|---------------|
| 0 (sharp) | 30 | 1.0 | 20 | Banyak corner, polygon-y |
| 4 (default) | 60 | 4.0 | 45 | Balanced cartoon |
| 7 (smooth) | 120 | 8.0 | 90 | Smooth Bezier, less detail |
| 10 (max) | 180 | 10.0 | 180 | Very smooth, simplified |

(Approximate mapping; tune empirically saat Phase 4.)

---

**End of PRD v0.1.**
