/**
 * Real backend client for Phase 5a axum API.
 * Endpoints match docs/PRD.md §11 Phase 5.
 */

import type {
  AnimPreviewOptions,
  AnimPreviewResponse,
  AtlasPackOptions,
  AtlasPackResponse,
  AudioOptions,
  AudioResponse,
  BatchResponse,
  BgRemoveOptions,
  HealthResponse,
  NineSliceOptions,
  NineSliceResponse,
  OptimizeOptions,
  OptimizeResponse,
  Preset,
  ProgressCallback,
  ScaleOptions,
  ScaleResponse,
  SvgOptimizeOptions,
  SvgOptimizeResponse,
  ToolId,
  TrimPadOptions,
  TrimPadResponse,
  VectorizeOptions,
  VectorizeResponse,
  VideoToSpriteOptions,
  VideoToSpriteResponse,
} from '~/types/pixiekit'

export interface ApiClient {
  health(): Promise<HealthResponse>
  processBgRemove(
    input: string,
    output: string,
    options: BgRemoveOptions,
    onProgress?: ProgressCallback,
  ): Promise<BatchResponse>
  /** Single-file upload mode — returns processed image bytes for client-side download. */
  processBgRemoveUpload(file: File, options: BgRemoveOptions): Promise<Blob>
  processVectorize(
    input: string,
    output: string,
    options: VectorizeOptions,
    onProgress?: ProgressCallback,
  ): Promise<VectorizeResponse>
  /** Single-file upload — returns SVG blob (image/svg+xml). */
  processVectorizeUpload(file: File, options: VectorizeOptions): Promise<Blob>
  processVideoToSprite(
    input: string,
    output: string,
    options: VideoToSpriteOptions,
    onProgress?: ProgressCallback,
  ): Promise<VideoToSpriteResponse>
  /** Single-file upload — returns sprite-sheet blob plus meta from response headers. */
  processVideoToSpriteUpload(
    file: File,
    options: VideoToSpriteOptions,
  ): Promise<VideoToSpriteUploadResult>
  processAtlasPack(
    input: string,
    output: string,
    options: AtlasPackOptions,
  ): Promise<AtlasPackResponse>
  processOptimize(
    input: string,
    output: string,
    options: OptimizeOptions,
    onProgress?: ProgressCallback,
  ): Promise<OptimizeResponse>
  processScale(
    input: string,
    output: string,
    options: ScaleOptions,
    onProgress?: ProgressCallback,
  ): Promise<ScaleResponse>
  processAudio(
    input: string,
    output: string,
    options: AudioOptions,
    onProgress?: ProgressCallback,
  ): Promise<AudioResponse>
  processTrimPad(
    input: string,
    output: string,
    options: TrimPadOptions,
    onProgress?: ProgressCallback,
  ): Promise<TrimPadResponse>
  processSvgOptimize(
    input: string,
    output: string,
    options: SvgOptimizeOptions,
    onProgress?: ProgressCallback,
  ): Promise<SvgOptimizeResponse>
  processNineSlice(
    input: string,
    output: string,
    options: NineSliceOptions,
    onProgress?: ProgressCallback,
  ): Promise<NineSliceResponse>
  processNineSliceUpload(file: File, options: NineSliceOptions): Promise<Blob>
  processAnimPreview(
    input: string,
    output: string,
    options: AnimPreviewOptions,
    onProgress?: ProgressCallback,
  ): Promise<AnimPreviewResponse>
  processAnimPreviewUpload(file: File, options: AnimPreviewOptions): Promise<Blob>

  /** List all presets across tools (filter by `tool` field at the call site). */
  listPresets(): Promise<Preset<unknown>[]>
  getPreset(name: string): Promise<Preset<unknown> | null>
  savePreset(name: string, tool: ToolId, options: unknown): Promise<Preset<unknown>>
  deletePreset(name: string): Promise<void>
}

export interface VideoToSpriteUploadResult {
  blob: Blob
  frameCount?: number
  fps?: number
  frameSize?: number
}

class HttpError extends Error {
  constructor(public status: number, message: string) {
    super(message)
    this.name = 'HttpError'
  }
}

function buildUrl(baseUrl: string, path: string): string {
  return `${baseUrl.replace(/\/$/, '')}${path}`
}

async function postJson<TReq, TRes>(
  baseUrl: string,
  path: string,
  body: TReq,
): Promise<TRes> {
  const res = await fetch(buildUrl(baseUrl, path), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  })
  if (!res.ok) {
    const text = await res.text().catch(() => res.statusText)
    throw new HttpError(res.status, `${path} → ${res.status}: ${text}`)
  }
  return res.json() as Promise<TRes>
}

async function putJson<TReq, TRes>(
  baseUrl: string,
  path: string,
  body: TReq,
): Promise<TRes> {
  const res = await fetch(buildUrl(baseUrl, path), {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  })
  if (!res.ok) {
    const text = await res.text().catch(() => res.statusText)
    throw new HttpError(res.status, `${path} → ${res.status}: ${text}`)
  }
  return res.json() as Promise<TRes>
}

interface ListPresetsResponse {
  presets: Array<{ name: string; tool: string; version: number; options: unknown }>
  presets_dir: string
}

interface PresetResponse {
  name: string
  tool: string
  version: number
  options: unknown
}

function toFrontendPreset(p: PresetResponse): Preset<unknown> {
  return {
    name: p.name,
    tool: p.tool as ToolId,
    options: p.options,
  }
}

export function createApiClient(baseUrl: string): ApiClient {
  return {
    async health() {
      const res = await fetch(buildUrl(baseUrl, '/api/health'))
      if (!res.ok) throw new HttpError(res.status, `health → ${res.status}`)
      return res.json() as Promise<HealthResponse>
    },

    async processBgRemove(input, output, options, _onProgress) {
      // Real backend currently doesn't stream progress over fetch — we'd need
      // SSE or websocket. For now, await the final result. Phase 6 enhancement.
      return postJson<unknown, BatchResponse>(baseUrl, '/api/bg-remove', {
        input,
        output,
        options,
      })
    },

    async processBgRemoveUpload(file, options) {
      const fd = new FormData()
      fd.append('file', file, file.name)
      // Backend ApiOptions uses `format` (not `output_format`) and ignores
      // `webp_quality` — remap before sending.
      fd.append(
        'options',
        JSON.stringify({
          target_color: options.target_color,
          fuzz: options.fuzz,
          despill: options.despill,
          erode: options.erode,
          format: options.output_format,
        }),
      )
      const res = await fetch(buildUrl(baseUrl, '/api/bg-remove'), {
        method: 'POST',
        body: fd,
      })
      if (!res.ok) {
        const text = await res.text().catch(() => res.statusText)
        throw new HttpError(res.status, `bg-remove upload → ${res.status}: ${text}`)
      }
      return res.blob()
    },

    async processVectorize(input, output, options, _onProgress) {
      return postJson<unknown, VectorizeResponse>(baseUrl, '/api/vectorize', {
        input,
        output,
        options,
      })
    },

    async processVectorizeUpload(file, options) {
      const fd = new FormData()
      fd.append('file', file, file.name)
      // Backend ApiOptions uses `smooth` (not `smoothness`).
      const payload: Record<string, unknown> = {
        mode: options.mode,
        smooth: options.smoothness,
        filter_speckle: options.filter_speckle,
        color_precision: options.color_precision,
        layer_difference: options.layer_difference,
        corner_threshold: options.corner_threshold,
        length_threshold: options.length_threshold,
        splice_threshold: options.splice_threshold,
        path_precision: options.path_precision,
      }
      fd.append('options', JSON.stringify(payload))
      const res = await fetch(buildUrl(baseUrl, '/api/vectorize'), {
        method: 'POST',
        body: fd,
      })
      if (!res.ok) {
        const text = await res.text().catch(() => res.statusText)
        throw new HttpError(res.status, `vectorize upload → ${res.status}: ${text}`)
      }
      return res.blob()
    },

    async processVideoToSprite(input, output, options, _onProgress) {
      return postJson<unknown, VideoToSpriteResponse>(
        baseUrl,
        '/api/video-to-sprite',
        { input, output, options },
      )
    },

    async processVideoToSpriteUpload(file, options) {
      const fd = new FormData()
      fd.append('file', file, file.name)
      // Backend uses `format` (not `output_format`); chroma fields nest under `chroma_key`.
      const payload: Record<string, unknown> = {
        fps: options.fps,
        frame_size: options.frame_size,
        format: options.output_format,
        webp_quality: options.webp_quality,
      }
      if (options.chroma_key) {
        const hexToTriplet = (hex: string): [number, number, number] => {
          const t = hex.replace('#', '')
          if (t.length !== 6) return [0, 255, 0]
          return [
            parseInt(t.slice(0, 2), 16),
            parseInt(t.slice(2, 4), 16),
            parseInt(t.slice(4, 6), 16),
          ]
        }
        payload.chroma_key = {
          target_color: hexToTriplet(options.chroma_target),
          fuzz: options.chroma_fuzz,
          despill: options.chroma_despill,
          erode: options.chroma_erode,
        }
      }
      fd.append('options', JSON.stringify(payload))
      const res = await fetch(buildUrl(baseUrl, '/api/video-to-sprite'), {
        method: 'POST',
        body: fd,
      })
      if (!res.ok) {
        const text = await res.text().catch(() => res.statusText)
        throw new HttpError(res.status, `video-to-sprite upload → ${res.status}: ${text}`)
      }
      const blob = await res.blob()
      const num = (h: string | null): number | undefined => {
        if (h === null) return undefined
        const n = Number.parseInt(h, 10)
        return Number.isFinite(n) ? n : undefined
      }
      return {
        blob,
        frameCount: num(res.headers.get('x-pixiekit-frame-count')),
        fps: num(res.headers.get('x-pixiekit-fps')),
        frameSize: num(res.headers.get('x-pixiekit-frame-size')),
      }
    },

    async processAtlasPack(input, output, options) {
      return postJson<unknown, AtlasPackResponse>(baseUrl, '/api/atlas-pack', {
        input,
        output,
        options,
      })
    },

    async processOptimize(input, output, options, _onProgress) {
      return postJson<unknown, OptimizeResponse>(baseUrl, '/api/optimize', {
        input,
        output,
        options,
      })
    },

    async processScale(input, output, options, _onProgress) {
      return postJson<unknown, ScaleResponse>(baseUrl, '/api/scale', {
        input,
        output,
        options,
      })
    },

    async processAudio(input, output, options, _onProgress) {
      return postJson<unknown, AudioResponse>(baseUrl, '/api/audio', {
        input,
        output,
        options,
      })
    },

    async processTrimPad(input, output, options, _onProgress) {
      return postJson<unknown, TrimPadResponse>(baseUrl, '/api/trim-pad', {
        input,
        output,
        options,
      })
    },

    async processSvgOptimize(input, output, options, _onProgress) {
      return postJson<unknown, SvgOptimizeResponse>(baseUrl, '/api/svg-optimize', {
        input,
        output,
        options,
      })
    },
    async processNineSlice(input, output, options, _onProgress) {
      return postJson<unknown, NineSliceResponse>(baseUrl, '/api/nine-slice', {
        input,
        output,
        options,
      })
    },
    async processNineSliceUpload(file, options) {
      const fd = new FormData()
      fd.append('file', file, file.name)
      fd.append('options', JSON.stringify(options))
      const res = await fetch(buildUrl(baseUrl, '/api/nine-slice'), {
        method: 'POST',
        body: fd,
      })
      if (!res.ok) {
        const text = await res.text().catch(() => res.statusText)
        throw new HttpError(res.status, `nine-slice upload → ${res.status}: ${text}`)
      }
      return res.blob()
    },
    async processAnimPreview(input, output, options, _onProgress) {
      return postJson<unknown, AnimPreviewResponse>(baseUrl, '/api/anim-preview', {
        input,
        output,
        options,
      })
    },
    async processAnimPreviewUpload(file, options) {
      const fd = new FormData()
      fd.append('file', file, file.name)
      fd.append('options', JSON.stringify(options))
      const res = await fetch(buildUrl(baseUrl, '/api/anim-preview'), {
        method: 'POST',
        body: fd,
      })
      if (!res.ok) {
        const text = await res.text().catch(() => res.statusText)
        throw new HttpError(res.status, `anim-preview upload → ${res.status}: ${text}`)
      }
      return res.blob()
    },

    async listPresets() {
      const res = await fetch(buildUrl(baseUrl, '/api/presets'))
      if (!res.ok) throw new HttpError(res.status, `listPresets → ${res.status}`)
      const body = (await res.json()) as ListPresetsResponse
      return body.presets.map(toFrontendPreset)
    },

    async getPreset(name) {
      const res = await fetch(buildUrl(baseUrl, `/api/presets/${encodeURIComponent(name)}`))
      if (res.status === 404) return null
      if (!res.ok) throw new HttpError(res.status, `getPreset → ${res.status}`)
      const body = (await res.json()) as PresetResponse
      return toFrontendPreset(body)
    },

    async savePreset(name, tool, options) {
      const body = await putJson<unknown, PresetResponse>(
        baseUrl,
        `/api/presets/${encodeURIComponent(name)}`,
        { tool, options },
      )
      return toFrontendPreset(body)
    },

    async deletePreset(name) {
      const res = await fetch(buildUrl(baseUrl, `/api/presets/${encodeURIComponent(name)}`), {
        method: 'DELETE',
      })
      if (!res.ok && res.status !== 404) {
        throw new HttpError(res.status, `deletePreset → ${res.status}`)
      }
    },
  }
}
