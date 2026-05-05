# Pixiekit — Technical PRD

**Project:** Pixiekit (asset preparation toolkit untuk Dom Dom World / Ulin)
**Version:** 0.1 (initial)
**Last updated:** 2026-05-05
**Owner:** Mochammad Lutfi
**Status:** Pre-development — single source of truth untuk implementasi
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

#### 7.1.6 Exit codes

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

### Total estimasi

~7-10 hari kerja untuk Phase 1-6 lengkap. Phase 1 adalah blocker (rest depends on `core`). Phase 2-6 bisa parallel kalau punya 2 dev (kita cuma 1, jadi sequential).

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
