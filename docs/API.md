# Pixiekit Web API (Phase 5a)

REST API server (`pixiekit-web-api`) wrapping the `pixiekit-core` tools. This
document is the contract between the backend (this crate) and the SaaS
frontend (Phase 5b, Nuxt 4).

The API is intentionally small and stateless. There is no auth, no DB, no
session — the server is a thin HTTP layer over `pixiekit-core` plus a temp
filesystem for multipart uploads.

## Run

```bash
PORT=8765 cargo run --release --bin pixiekit-web-api
# or after build
./target/release/pixiekit-web-api
```

### Environment

| Variable | Default | Description |
|---|---|---|
| `HOST` | `0.0.0.0` | Bind address |
| `PORT` | `8765` | Listen port |
| `CORS_ALLOWED_ORIGINS` | `http://localhost:3000,http://localhost:5173` | Comma-separated allow-list |
| `RUST_LOG` | `info` | tracing-subscriber filter (e.g. `info,pixiekit_web_api=debug`) |

### Limits

- Request body: **100 MiB** (videos are large)
- Response time: bounded only by the operation; expect long-running for video

## Conventions

- All responses are JSON unless an endpoint explicitly streams a binary payload
  (multipart upload mode). Errors are always JSON: `{"error": "<message>"}`.
- HTTP status codes follow conventional REST mapping:
  - `200 OK` — success
  - `400 Bad Request` — malformed input (bad JSON, bad multipart, unsupported
    Content-Type, invalid filename)
  - `404 Not Found` — server-side path missing (Mode A) or unknown route
  - `422 Unprocessable Entity` — semantically valid input but processing
    failed (ffmpeg crash, no frames extracted, frame size mismatch)
  - `500 Internal Server Error` — unexpected I/O or encoder failure
  - `501 Not Implemented` — vectorize stub (Phase 3 not merged yet)
  - `503 Service Unavailable` — `ffmpeg` not installed on the server

## Endpoints

### `GET /api/health`

Liveness probe.

**Response 200**:

```json
{"status": "ok", "version": "0.1.0"}
```

---

### `POST /api/bg-remove`

Background removal (chroma key + despill + alpha erode).

Two modes — dispatched on `Content-Type`:

#### Mode A — `application/json` (server-side path batch)

For power users / scripts running on the same machine as the server.

**Request body**:

```json
{
  "input": "/abs/path/to/folder-or-file",
  "output": "/abs/path/to/output-folder",
  "options": {
    "target_color": "#00FF00",
    "fuzz": 0.35,
    "despill": true,
    "erode": 1,
    "format": "png"
  }
}
```

| Field | Type | Default | Notes |
|---|---|---|---|
| `input` | string (absolute path) | — | File or directory. Subdirs are NOT walked (depth 1). |
| `output` | string (absolute path) | — | Created if missing. Existing files are overwritten. |
| `options.target_color` | hex string OR `[u8; 3]` | `[0, 255, 0]` | `"#00FF00"` and `[0, 255, 0]` are equivalent. |
| `options.fuzz` | f32 (0.0 – 1.0) | `0.35` | Threshold as fraction of max RGB Euclidean distance. |
| `options.despill` | bool | `true` | Clamp green channel: `g = min(g, max(r, b))`. |
| `options.erode` | u8 (0 – 5) | `1` | Diamond:1 alpha erode iterations. Clamped to 5. |
| `options.format` | `"png"` \| `"webp"` | `"png"` | Output container. |

**Response 200**:

```json
{
  "processed": 32,
  "failed": 0,
  "duration_ms": 1532,
  "files": [
    {"input": "/abs/in/a.png", "output": "/abs/out/a.png", "status": "ok", "error": null},
    {"input": "/abs/in/b.jpg", "output": null, "status": "failed", "error": "..."}
  ]
}
```

Empty input directory returns `200` with `processed: 0` and `files: []`.

#### Mode B — `multipart/form-data` (browser upload)

Single image at a time. Designed for a SaaS frontend uploading one file from
a `<input type="file">`.

**Multipart fields**:

| Field | Required | Description |
|---|---|---|
| `file` | yes | The image (PNG/JPG/WebP). Filename is honoured but path-stripped. |
| `options` | no | JSON string matching the `options` object from Mode A. Omitted → defaults. |

**Response 200**: binary image stream.

| Header | Value |
|---|---|
| `Content-Type` | `image/png` (default) or `image/webp` if `options.format == "webp"` |

---

### `POST /api/video-to-sprite`

Extract frames from a video and stitch into a horizontal sprite sheet.
Optionally apply chroma key per frame.

Requires `ffmpeg` on `PATH`; otherwise returns `503 Service Unavailable`.

#### Mode A — `application/json` (server-side path batch)

**Request body**:

```json
{
  "input": "/abs/path/to/folder-or-video",
  "output": "/abs/path/to/output-folder",
  "options": {
    "fps": 8,
    "frame_size": 256,
    "format": "webp",
    "webp_quality": 90,
    "chroma_key": {
      "target_color": [0, 255, 0],
      "fuzz": 0.35,
      "despill": true,
      "erode": 1
    }
  }
}
```

| Field | Type | Default | Notes |
|---|---|---|---|
| `input` | string (absolute path) | — | Video file or folder of videos (`.mp4`, `.mov`, `.webm`). |
| `output` | string (absolute path) | — | Created if missing. |
| `options.fps` | u8 (1 – 30) | `8` | Output frame rate. |
| `options.frame_size` | u32 (16 – 4096) | `256` | Square frame edge length. |
| `options.format` | `"png"` \| `"webp"` | `"webp"` | Sprite container. |
| `options.webp_quality` | u8 (0 – 100) | `90` | Ignored for PNG. Alpha is always lossless. |
| `options.chroma_key` | object (optional) | absent | Same shape as `bg-remove` options (no `format`). |

**Response 200**:

```json
{
  "processed": 1,
  "failed": 0,
  "duration_ms": 8123,
  "files": [
    {
      "input": "/abs/in/wave.mp4",
      "sprite": "/abs/out/wave.webp",
      "metadata": "/abs/out/wave.json",
      "frame_count": 24,
      "frame_size": 256,
      "status": "ok",
      "error": null
    }
  ]
}
```

#### Mode B — `multipart/form-data`

Single video at a time. Saves to a server-managed temp dir, processes, and
returns the sprite sheet bytes inline. Metadata is exposed via response
headers.

**Multipart fields**:

| Field | Required | Description |
|---|---|---|
| `file` | yes | The video (`.mp4`, `.mov`, `.webm`). |
| `options` | no | JSON string of the `options` object. |

**Response 200**: binary sprite stream.

| Header | Value |
|---|---|
| `Content-Type` | `image/webp` (default) or `image/png` |
| `X-Pixiekit-Frame-Count` | integer, e.g. `24` |
| `X-Pixiekit-FPS` | integer, e.g. `8` |
| `X-Pixiekit-Frame-Size` | integer, e.g. `256` |

The temp upload + temp output directory are deleted after the response is
written.

---

### `POST /api/vectorize`

**Status: stub (501).** Returns the same body regardless of input:

```json
{"error": "vectorize endpoint will be available after Phase 3 merge"}
```

After Phase 3 (`core::vectorize`) merges to `main`, this handler will be
extended to mirror `bg-remove` (Mode A + Mode B) and produce SVG output.

---

## Error response shape

Every non-2xx response (including `501`) returns the same envelope:

```json
{"error": "<human-readable message>"}
```

Frontend code can rely on `response.error` being a string when
`response.status >= 400`.

## Examples

### curl: health

```bash
curl http://localhost:8765/api/health
```

### curl: Mode A bg-remove

```bash
curl -X POST http://localhost:8765/api/bg-remove \
  -H "Content-Type: application/json" \
  -d '{
    "input": "/Users/me/raw",
    "output": "/Users/me/clean",
    "options": {"target_color": "#00FF00", "fuzz": 0.35, "format": "png"}
  }'
```

### curl: Mode B bg-remove (upload)

```bash
curl -X POST http://localhost:8765/api/bg-remove \
  -F 'file=@./char.png' \
  -F 'options={"target_color":"#00FF00","fuzz":0.35,"format":"webp"}' \
  --output cleaned.webp
```

### curl: Mode B video-to-sprite (upload)

```bash
curl -X POST http://localhost:8765/api/video-to-sprite \
  -F 'file=@./wave.mp4' \
  -F 'options={"fps":8,"frame_size":256,"format":"webp"}' \
  --output sprite.webp -D headers.txt
grep -i 'x-pixiekit' headers.txt
```

## Notes for frontend implementers

- Always set `Accept: application/json` for Mode A. Multipart Mode B returns
  binary; either treat the response as a `Blob` (browser `fetch`) or use a
  binary download stream.
- The 100 MiB body limit is enforced by `tower-http`'s `RequestBodyLimitLayer`.
  Larger uploads receive `413 Payload Too Large` from the framework.
- CORS must be configured at server start (`CORS_ALLOWED_ORIGINS`); preflight
  requests are handled automatically.
- Because this server is offline-first (single host, no DB), there is no rate
  limiting. Put a reverse proxy in front of it for production.
