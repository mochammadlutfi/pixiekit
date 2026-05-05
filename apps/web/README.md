# Pixiekit — SaaS Frontend (Phase 5b)

Nuxt 4 + Vue 3 + TypeScript + Tailwind dashboard for the three Pixiekit tools:
BG Remove, Vectorize, and Video → Sprite.

## Develop

```bash
cd apps/web
pnpm install
pnpm dev
# → http://localhost:3000
```

By default the frontend runs against a **mock backend** (`app/lib/mock-api.ts`)
so you can iterate on UI without the Rust backend running. To target the real
Phase 5a axum API:

```bash
VITE_PIXIEKIT_API_URL=http://localhost:8787 pnpm dev
```

The badge in the top-right corner shows whether the app is in `mock` or `real`
mode at runtime.

## Build

```bash
pnpm typecheck      # vue-tsc strict (run via nuxt prepare + tsc)
pnpm build          # nuxt build (.output/)
pnpm preview        # serve the production build
```

## Layout

```
app/
├── app.vue                    # navigation + page outlet
├── pages/
│   ├── index.vue              # tool launcher (3 cards)
│   ├── bg-remove.vue
│   ├── vectorize.vue
│   └── video-to-sprite.vue
├── components/
│   ├── ToolHeader.vue
│   ├── FileDropZone.vue
│   ├── BeforeAfterPreview.vue
│   ├── SettingsPanel.vue
│   ├── ProgressLog.vue
│   └── PathInput.vue
├── composables/
│   ├── usePixiekitApi.ts      # mock vs real switch
│   └── useToolPreset.ts       # localStorage preset save/load
├── lib/
│   ├── api-client.ts          # real backend client
│   └── mock-api.ts            # standalone-dev mock
├── types/pixiekit.ts          # shared API types
└── assets/css/tailwind.css
```

## Environment

| Variable                  | Effect                                                    |
| ------------------------- | --------------------------------------------------------- |
| `VITE_PIXIEKIT_API_URL`   | When set, the frontend calls the real backend.            |

## Notes

- **Folder picker** uses the File System Access API in Chrome/Edge. In other
  browsers (Safari), users paste a path manually — Cmd+Opt+C in Finder copies
  the absolute path to a selected item.
- **Recent paths** are stored under `pixiekit:recent:*` in localStorage, max
  five entries per field.
- **Presets** are stored under `pixiekit:presets:v1`. Each tool ships a
  default preset on first load.
