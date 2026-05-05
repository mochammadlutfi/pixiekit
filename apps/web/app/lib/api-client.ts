/**
 * Real backend client for Phase 5a axum API.
 * Endpoints match docs/PRD.md §11 Phase 5.
 */

import type {
  BatchResponse,
  BgRemoveOptions,
  HealthResponse,
  ProgressCallback,
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
  }
}
