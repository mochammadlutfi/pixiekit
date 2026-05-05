/**
 * Real backend client for Phase 5a axum API.
 * Endpoints match docs/PRD.md §11 Phase 5.
 */

import type {
  BatchResponse,
  BgRemoveOptions,
  HealthResponse,
  Preset,
  ProgressCallback,
  ToolId,
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
  processVectorize(
    input: string,
    output: string,
    options: VectorizeOptions,
    onProgress?: ProgressCallback,
  ): Promise<VectorizeResponse>
  processVideoToSprite(
    input: string,
    output: string,
    options: VideoToSpriteOptions,
    onProgress?: ProgressCallback,
  ): Promise<VideoToSpriteResponse>

  /** List all presets across tools (filter by `tool` field at the call site). */
  listPresets(): Promise<Preset<unknown>[]>
  getPreset(name: string): Promise<Preset<unknown> | null>
  savePreset(name: string, tool: ToolId, options: unknown): Promise<Preset<unknown>>
  deletePreset(name: string): Promise<void>
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

    async processVectorize(input, output, options, _onProgress) {
      return postJson<unknown, VectorizeResponse>(baseUrl, '/api/vectorize', {
        input,
        output,
        options,
      })
    },

    async processVideoToSprite(input, output, options, _onProgress) {
      return postJson<unknown, VideoToSpriteResponse>(
        baseUrl,
        '/api/video-to-sprite',
        { input, output, options },
      )
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
