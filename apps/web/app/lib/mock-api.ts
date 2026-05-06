/**
 * Mock backend that simulates Phase 5a axum responses.
 * Used when VITE_PIXIEKIT_API_URL is not set, for standalone frontend dev.
 *
 * Presets in mock mode persist in localStorage under `pixiekit:presets:v2` so
 * the on-disk shape (a flat list across tools) matches the real backend.
 */

import type { ApiClient } from '~/lib/api-client'
import type {
  AtlasPackOptions,
  AtlasPackResponse,
  AudioOptions,
  AudioResponse,
  BatchResponse,
  BgRemoveOptions,
  HealthResponse,
  OptimizeOptions,
  OptimizedFile,
  OptimizeResponse,
  NineSliceOptions,
  NineSliceResponse,
  AnimPreviewOptions,
  AnimPreviewResponse,
  Preset,
  ProcessedFile,
  ProgressCallback,
  ScaleOptions,
  ScaledFile,
  ScaleResponse,
  SvgOptimizeFile,
  SvgOptimizeOptions,
  SvgOptimizeResponse,
  ToolId,
  TrimPadFile,
  TrimPadOptions,
  TrimPadResponse,
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

    async processBgRemoveUpload(
      file: File,
      _options: BgRemoveOptions,
    ): Promise<Blob> {
      // Mock: pretend to process by echoing the input bytes after a small delay.
      await sleep(STEP_MS * 2)
      return file.slice(0, file.size, file.type || 'image/png')
    },

    async processVectorizeUpload(
      file: File,
      _options: VectorizeOptions,
    ): Promise<Blob> {
      await sleep(STEP_MS * 3)
      // Mock: emit a tiny SVG indicating mock mode + filename.
      const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256" viewBox="0 0 256 256">
  <rect width="256" height="256" fill="#FFB88C"/>
  <text x="50%" y="48%" font-family="ui-sans-serif" font-size="14" fill="#3D3D3D" text-anchor="middle" dominant-baseline="middle">${file.name}</text>
  <text x="50%" y="58%" font-family="ui-sans-serif" font-size="11" fill="#3D3D3D" text-anchor="middle" dominant-baseline="middle" opacity="0.7">mock svg</text>
</svg>`
      return new Blob([svg], { type: 'image/svg+xml' })
    },

    async processVideoToSpriteUpload(
      file: File,
      options: VideoToSpriteOptions,
    ) {
      // Pretend to extract & stitch frames. Echo a placeholder image.
      await sleep(STEP_MS * 6)
      const totalFrames = 32
      const sprite = `<svg xmlns="http://www.w3.org/2000/svg" width="${options.frame_size * 4}" height="${options.frame_size}" viewBox="0 0 ${options.frame_size * 4} ${options.frame_size}">
  <rect width="100%" height="100%" fill="#A8D8EA"/>
  <text x="50%" y="50%" font-family="ui-sans-serif" font-size="20" fill="#3D3D3D" text-anchor="middle" dominant-baseline="middle">${totalFrames} frames @ ${options.fps}fps · ${file.name}</text>
</svg>`
      return {
        blob: new Blob([sprite], { type: 'image/svg+xml' }),
        frameCount: totalFrames,
        fps: options.fps,
        frameSize: options.frame_size,
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

    async processAtlasPack(
      _input: string,
      output: string,
      options: AtlasPackOptions,
    ): Promise<AtlasPackResponse> {
      const start = Date.now()
      await sleep(STEP_MS * 4)
      const ext = options.format === 'webp' ? 'webp' : 'png'
      const name = options.name || 'atlas'
      return {
        atlas_path: `${output}/${name}.${ext}`,
        metadata_path: `${output}/${name}.json`,
        packed: 8,
        total: 8,
        atlas_size: { w: 512, h: 512 },
        efficiency: 0.82,
        duration_ms: Date.now() - start,
      }
    },

    async processOptimize(
      input: string,
      output: string,
      options: OptimizeOptions,
      onProgress?: ProgressCallback,
    ): Promise<OptimizeResponse> {
      const start = Date.now()
      const ext = options.target_format === 'png' ? 'png' : 'webp'
      const files: OptimizedFile[] = []
      for (let i = 0; i < MOCK_FILES.length; i++) {
        await sleep(STEP_MS / 2)
        const original = MOCK_FILES[i]!
        const stem = original.replace(/\.png$/, '')
        const inputSize = 200_000 + i * 5000
        const outputSize = Math.round(inputSize * (options.lossless ? 0.65 : 0.4))
        files.push({
          input_path: `${input}/${original}`,
          output_path: `${output}/${stem}.${ext}`,
          duration_ms: STEP_MS / 2,
          ok: true,
          input_size: inputSize,
          output_size: outputSize,
          ratio: outputSize / inputSize,
        })
        onProgress?.({
          index: i + 1,
          total: MOCK_FILES.length,
          current_file: original,
          duration_ms: STEP_MS / 2,
        })
      }
      return {
        processed: files.length,
        failed: 0,
        duration_ms: Date.now() - start,
        files,
      }
    },

    async processScale(
      input: string,
      output: string,
      options: ScaleOptions,
      onProgress?: ProgressCallback,
    ): Promise<ScaleResponse> {
      const start = Date.now()
      const files: ScaledFile[] = []
      for (let i = 0; i < MOCK_FILES.length; i++) {
        await sleep(STEP_MS / 2)
        const original = MOCK_FILES[i]!
        const stem = original.replace(/\.png$/, '')
        const variants = options.target_scales.map(s => {
          const label = `${s}`
          if (options.naming === 'flutter') {
            return `${output}/${label}x/${original}`
          }
          if (options.naming === 'nested') {
            return `${output}/${label}/${original}`
          }
          return s === 1.0
            ? `${output}/${original}`
            : `${output}/${stem}@${label}x.png`
        })
        files.push({
          input: `${input}/${original}`,
          variants,
          status: 'ok',
        })
        onProgress?.({
          index: i + 1,
          total: MOCK_FILES.length,
          current_file: original,
          duration_ms: STEP_MS / 2,
        })
      }
      return {
        processed: files.length,
        failed: 0,
        duration_ms: Date.now() - start,
        files,
      }
    },

    async processAudio(
      input: string,
      output: string,
      options: AudioOptions,
      onProgress?: ProgressCallback,
    ): Promise<AudioResponse> {
      const mockAudioFiles = ['ambient.wav', 'sfx_jump.wav', 'voice_intro.mp3']
      const start = Date.now()
      const files: ProcessedFile[] = []
      for (let i = 0; i < mockAudioFiles.length; i++) {
        await sleep(STEP_MS)
        const f = mockAudioFiles[i]!
        const stem = f.replace(/\.[^.]+$/, '')
        files.push({
          input_path: `${input}/${f}`,
          output_path: `${output}/${stem}.${options.target_format}`,
          duration_ms: STEP_MS,
          ok: true,
        })
        onProgress?.({
          index: i + 1,
          total: mockAudioFiles.length,
          current_file: f,
          duration_ms: STEP_MS,
        })
      }
      return {
        processed: files.length,
        failed: 0,
        duration_ms: Date.now() - start,
        files,
        duration_ms_in: 4200,
        duration_ms_out: options.trim_silence ? 3650 : 4200,
        integrated_lufs: options.normalize ? options.target_lufs : null,
      }
    },

    async processTrimPad(
      input: string,
      output: string,
      _options: TrimPadOptions,
      onProgress?: ProgressCallback,
    ): Promise<TrimPadResponse> {
      const { processedFiles, durationMs } = await simulateBatch(
        MOCK_FILES,
        onProgress,
        input,
        output,
      )
      const files: TrimPadFile[] = processedFiles.map((f, i) => ({
        ...f,
        output_size: [200 + i * 4, 200 + i * 4],
        bbox: [10, 10, 200 + i * 4, 200 + i * 4],
      }))
      return {
        processed: files.length,
        failed: 0,
        duration_ms: durationMs,
        files,
      }
    },

    async processSvgOptimize(
      input: string,
      output: string,
      _options: SvgOptimizeOptions,
      onProgress?: ProgressCallback,
    ): Promise<SvgOptimizeResponse> {
      const svgFiles = MOCK_FILES.slice(0, 4).map(f => f.replace(/\.png$/, '.svg'))
      const { processedFiles, durationMs } = await simulateBatch(
        svgFiles,
        onProgress,
        input,
        output,
      )
      const files: SvgOptimizeFile[] = processedFiles.map((f, i) => ({
        ...f,
        input_size: 4096 + i * 512,
        output_size: 1024 + i * 128,
        ratio: 0.25,
      }))
      return {
        processed: files.length,
        failed: 0,
        duration_ms: durationMs,
        files,
        svg_data_uri: dataUriPlaceholder('Minified SVG', '#C5E1A5'),
      }
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

    async processNineSlice(
      input: string,
      output: string,
      _options: NineSliceOptions,
      onProgress?: ProgressCallback,
    ): Promise<NineSliceResponse> {
      const { processedFiles, durationMs } = await simulateBatch(
        MOCK_FILES.slice(0, 3),
        onProgress,
        input,
        output,
      )
      return {
        processed: processedFiles.length,
        failed: 0,
        duration_ms: durationMs,
        files: processedFiles.map(f => ({
          ...f,
          outputs: [`${f.output_path}_top.png`, `${f.output_path}_center.png`],
        })),
      }
    },

    async processNineSliceUpload(
      file: File,
      options: NineSliceOptions,
    ): Promise<Blob> {
      await sleep(STEP_MS * 2)
      if (options.mode === 'metadata') {
        const metadata = {
          image: file.name,
          size: { w: 100, h: 100 },
          slices: {
            top: options.top,
            right: options.right,
            bottom: options.bottom,
            left: options.left,
          },
        }
        return new Blob([JSON.stringify(metadata, null, 2)], {
          type: 'application/json',
        })
      }
      // For split modes, mock a zip (actually just the file for now)
      return file.slice(0, file.size, 'application/zip')
    },

    async processAnimPreview(
      input: string,
      output: string,
      options: AnimPreviewOptions,
      onProgress?: ProgressCallback,
    ): Promise<AnimPreviewResponse> {
      const { processedFiles, durationMs } = await simulateBatch(
        ['animation'],
        onProgress,
        input,
        output,
      )
      return {
        processed: processedFiles.length,
        failed: 0,
        duration_ms: durationMs,
        files: processedFiles.map(f => ({
          ...f,
          output: `${output}/animation.${options.format}`,
        })),
      }
    },

    async processAnimPreviewUpload(
      file: File,
      options: AnimPreviewOptions,
    ): Promise<Blob> {
      await sleep(STEP_MS * 4)
      const mime = options.format === 'gif' ? 'image/gif' : `video/${options.format}`
      // Mock: we can't easily generate a video, so we just return a placeholder blob with the right mime
      return file.slice(0, file.size, mime)
    },
  }
}
