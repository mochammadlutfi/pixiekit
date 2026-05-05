/**
 * Mock backend that simulates Phase 5a axum responses.
 * Used when VITE_PIXIEKIT_API_URL is not set, for standalone frontend dev.
 *
 * Presets in mock mode persist in localStorage under `pixiekit:presets:v2` so
 * the on-disk shape (a flat list across tools) matches the real backend.
 */

import type { ApiClient } from '~/lib/api-client'
import type {
  BatchResponse,
  BgRemoveOptions,
  HealthResponse,
  Preset,
  ProcessedFile,
  ProgressCallback,
  ToolId,
  VectorizeOptions,
  VectorizeResponse,
  VideoToSpriteOptions,
  VideoToSpriteResponse,
} from '~/types/pixiekit'

const PRESET_STORAGE_KEY = 'pixiekit:presets:v2'

function readPresetStore(): Preset<unknown>[] {
  if (typeof localStorage === 'undefined') return []
  try {
    const raw = localStorage.getItem(PRESET_STORAGE_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw) as Preset<unknown>[]
    return Array.isArray(parsed) ? parsed : []
  } catch {
    return []
  }
}

function writePresetStore(presets: Preset<unknown>[]): void {
  if (typeof localStorage === 'undefined') return
  localStorage.setItem(PRESET_STORAGE_KEY, JSON.stringify(presets))
}

const MOCK_FILES = [
  'wave_01.png',
  'wave_02.png',
  'idle_01.png',
  'idle_02.png',
  'idle_03.png',
  'happy_01.png',
  'sad_01.png',
  'jump_01.png',
] as const

const STEP_MS = 100

function sleep(ms: number): Promise<void> {
  return new Promise(r => setTimeout(r, ms))
}

function dataUriPlaceholder(text: string, color = '#A8D8EA'): string {
  // 1×1 transparent PNG, kept tiny so it works without external assets
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256" viewBox="0 0 256 256">
    <rect width="256" height="256" fill="${color}"/>
    <text x="50%" y="50%" font-family="ui-sans-serif" font-size="14" fill="#3D3D3D" text-anchor="middle" dominant-baseline="middle">${text}</text>
  </svg>`
  return `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`
}

async function simulateBatch(
  files: readonly string[],
  onProgress: ProgressCallback | undefined,
  inputDir: string,
  outputDir: string,
): Promise<{ processedFiles: ProcessedFile[]; durationMs: number }> {
  const start = Date.now()
  const processedFiles: ProcessedFile[] = []
  for (let i = 0; i < files.length; i++) {
    await sleep(STEP_MS)
    const f = files[i]!
    const file: ProcessedFile = {
      input_path: `${inputDir}/${f}`,
      output_path: `${outputDir}/${f}`,
      duration_ms: STEP_MS,
      ok: true,
    }
    processedFiles.push(file)
    onProgress?.({
      index: i + 1,
      total: files.length,
      current_file: f,
      duration_ms: STEP_MS,
    })
  }
  return { processedFiles, durationMs: Date.now() - start }
}

export function createMockApiClient(): ApiClient {
  return {
    async health(): Promise<HealthResponse> {
      await sleep(50)
      return { status: 'ok', version: 'mock-0.1.0' }
    },

    async processBgRemove(
      input: string,
      output: string,
      _options: BgRemoveOptions,
      onProgress?: ProgressCallback,
    ): Promise<BatchResponse> {
      const { processedFiles, durationMs } = await simulateBatch(
        MOCK_FILES,
        onProgress,
        input,
        output,
      )
      return {
        processed: processedFiles.length,
        failed: 0,
        duration_ms: durationMs,
        files: processedFiles,
      }
    },

    async processVectorize(
      input: string,
      output: string,
      _options: VectorizeOptions,
      onProgress?: ProgressCallback,
    ): Promise<VectorizeResponse> {
      const svgFiles = MOCK_FILES.slice(0, 4).map(f => f.replace(/\.png$/, '.svg'))
      const { processedFiles, durationMs } = await simulateBatch(
        svgFiles,
        onProgress,
        input,
        output,
      )
      return {
        processed: processedFiles.length,
        failed: 0,
        duration_ms: durationMs,
        files: processedFiles,
        svg_data_uri: dataUriPlaceholder('SVG preview', '#FFB88C'),
      }
    },

    async listPresets(): Promise<Preset<unknown>[]> {
      await sleep(20)
      return readPresetStore()
    },

    async getPreset(name: string): Promise<Preset<unknown> | null> {
      await sleep(20)
      return readPresetStore().find(p => p.name === name) ?? null
    },

    async savePreset(
      name: string,
      tool: ToolId,
      options: unknown,
    ): Promise<Preset<unknown>> {
      await sleep(20)
      const trimmed = name.trim()
      const without = readPresetStore().filter(p => p.name !== trimmed)
      const next: Preset<unknown> = {
        name: trimmed,
        tool,
        options,
        created_at: Date.now(),
      }
      writePresetStore([...without, next])
      return next
    },

    async deletePreset(name: string): Promise<void> {
      await sleep(20)
      writePresetStore(readPresetStore().filter(p => p.name !== name))
    },

    async processVideoToSprite(
      input: string,
      output: string,
      options: VideoToSpriteOptions,
      onProgress?: ProgressCallback,
    ): Promise<VideoToSpriteResponse> {
      // Video processing simulated as 1 result file but slower per "frame" tick.
      const totalFrames = 32
      const start = Date.now()
      for (let i = 0; i < totalFrames; i++) {
        await sleep(STEP_MS / 2)
        onProgress?.({
          index: i + 1,
          total: totalFrames,
          current_file: `frame_${String(i + 1).padStart(4, '0')}.png`,
          duration_ms: STEP_MS / 2,
        })
      }
      const durationMs = Date.now() - start
      const ext = options.output_format === 'webp' ? 'webp' : 'png'
      const out: ProcessedFile = {
        input_path: `${input}/wave.mp4`,
        output_path: `${output}/wave.${ext}`,
        duration_ms: durationMs,
        ok: true,
      }
      return {
        processed: 1,
        failed: 0,
        duration_ms: durationMs,
        files: [out],
        frame_count: totalFrames,
        frame_size: options.frame_size,
        fps: options.fps,
        sprite_url: dataUriPlaceholder(
          `${totalFrames} frames @ ${options.fps}fps`,
          '#A8D8EA',
        ),
      }
    },
  }
}
