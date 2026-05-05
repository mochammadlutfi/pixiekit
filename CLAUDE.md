# CLAUDE.md

This file gives Claude Code (and other AI agents) the context needed to work on
this project across sessions. Read this **first** every new session.

## Project

**Pixiekit** — local Rust toolkit for game/animation asset preparation.

Single source of truth: [`docs/PRD.md`](docs/PRD.md) (read sections 1-2, 5, 11
on first session; consult others as needed).

## Stack

- **Rust** stable (1.80+ minimum, currently developed against 1.95)
- **Edition 2021**
- Cargo workspace, multiple crates
- System dependency: `ffmpeg` (for video-to-sprite, Phase 2+)

No Tauri (deferred). No Python. ImageMagick CLI replaced by pure Rust `image` crate.

## Workspace layout

```
pixiekit/
├── Cargo.toml                  # workspace
├── crates/
│   ├── core/                   # 📦 LIB: pure logic, no I/O orchestration
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── error.rs        # AppError + Result alias
│   │       ├── batch.rs        # folder traversal helpers
│   │       └── bg_remove.rs    # chroma key + despill + erode
│   └── cli/                    # ⌨️ BIN: pixiekit-cli
│       └── src/
│           ├── main.rs
│           └── commands/
│               ├── mod.rs
│               └── bg_remove.rs
└── docs/PRD.md
```

Future crates (per roadmap):
- `crates/mcp/` — Phase 4 (MCP server, stdio transport)
- `crates/web-api/` — Phase 5 (axum SaaS backend)
- `apps/web/` — Phase 5 (Nuxt 4 frontend)

## Build / run / test

```bash
# From repo root
cargo check --workspace                      # fast type check
cargo build --workspace                      # debug build
cargo build --release --workspace            # release build (~10MB binary)
cargo test --workspace                       # all unit tests
cargo clippy --workspace --all-targets -- -D warnings   # lint
cargo fmt --all                              # format

# Run CLI in dev (long form via cargo)
cargo run --bin pixiekit-cli -- bg-remove --input ./test/raw --output ./test/out

# Or use release binary directly
./target/release/pixiekit-cli bg-remove --help
```

## Conventions

### Code style

- **Pure logic in `core`** — no I/O orchestration (no progress bars, no
  println). I/O wrappers belong in `cli`/`mcp`/`web-api`.
- **Error types**: `core` uses `thiserror` (`pixiekit_core::Error`). Frontends
  use `anyhow` for ergonomic error context.
- **Modules**: one tool per file (`bg_remove.rs`, `vectorize.rs`,
  `video_to_sprite.rs`). Keep files focused.
- **Tests**: inline `#[cfg(test)]` mod tests for unit; `tests/` folder for
  integration when needed.
- **No `unwrap`** in library code (`core`). CLI may use `unwrap` only for
  static-known invariants (e.g., `ProgressStyle::with_template(...).unwrap()`).
- **No `panic!`** in any production path.

### Naming

- Crate names: `pixiekit-core`, `pixiekit-cli`, `pixiekit-mcp`
- Binary names: `pixiekit-cli`, `pixiekit-mcp`
- Types: `UpperCamelCase`, modules: `snake_case`
- Public functions in `core` should accept `&RgbaImage` (or path) and return
  `Result<RgbaImage>` or owned values — no shared state.

### Algorithm parity

The BG remove algorithm must match `clean-bg.py` (ImageMagick reference) on a
per-pixel basis:

- Chroma key: Euclidean RGB distance ≤ `fuzz × sqrt(255² × 3)` → alpha = 0
- Despill: `g_new = min(g, max(r, b))` for non-transparent pixels
- Erode: Diamond:1 = center + 4 cardinal neighbors, take min, repeat N times

When making changes to the algorithm, run parity test against existing
`flutter/ulin/art-workshop/rive/clean/` outputs (manual visual check OK for
Phase 1; automated diff in Phase 6).

## Roadmap status

| Phase | Scope | Status | Branch |
|-------|-------|:------:|--------|
| 1 | core::bg_remove + cli | ✅ done | `main` |
| 2 | core::video_to_sprite + cli | ✅ done | `main` |
| 3 | core::vectorize + cli | ✅ done | `main` |
| 4 | MCP server (stdio) | ✅ done | `main` |
| 5a | SaaS axum backend | ✅ done | `main` |
| 5b | SaaS Nuxt 4 frontend | ✅ done | `main` |
| 6 | Polish — presets across CLI/MCP/web + humanized errors | ✅ done | `main` |
| 7 | (Optional) Tauri desktop | ⏳ deferred | — |

Phase 6 ships preset save/load uniformly across surfaces, all backed by
`pixiekit_core::preset` (JSON under `~/.config/pixiekit/presets/`, override
with `PIXIEKIT_CONFIG_DIR`):

- **CLI** — `pixiekit-cli preset {save,list,show,delete,path}` (PRD §7.1.5)
  plus `--config <PATH>` on each tool to load a preset's options (PRD §7.1.1).
- **MCP** — `list_presets` (real) + `get_preset` tools (PRD §7.3.2).
- **Web API** — `/api/presets` CRUD (GET list / GET :name / PUT :name /
  DELETE :name).
- **Nuxt frontend** — `useToolPreset` writes through `usePixiekitApi()`, so
  presets sync with the backend in real mode and mirror in localStorage in
  mock mode.

M6.4 humanizes per-tool input preflight errors with single-line hints.

When starting a new phase, create a feature branch:
`git checkout -b feat/phase-N-{tool}`. Merge to `main` via PR (or fast-forward
for solo dev).

## Commit conventions

Conventional commits with scope:

```
feat(core): add vectorize module
feat(cli): add video-to-sprite subcommand
fix(core): correct erode boundary handling
docs(prd): update Phase 5 architecture
test(core): add chroma key parity tests
chore(deps): bump image to 0.25.6
```

Co-author footer is OK but not required.

## When stuck

1. Read [`docs/PRD.md`](docs/PRD.md) section relevant to the current phase
2. Check existing tests — they document expected behavior
3. For algorithm correctness: compare with `clean-bg.py` output on test
   fixture (`flutter/ulin/art-workshop/rive/raw/`)
4. For Rust-specific issues: `cargo check` first (fast feedback), then
   `cargo clippy` for idioms, then ask for help

## Out of scope reminders (do not propose)

- Tauri (deferred until Phase 6)
- Python rewrite of any logic (deprecated path)
- ImageMagick CLI shell-out (replaced by `image` crate)
- Cloud upload / multi-user / collaboration
- AI image generation (this tool post-processes, never generates)
- Mobile/iOS/Android targets
