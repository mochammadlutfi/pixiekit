import { invoke, Channel } from '@tauri-apps/api/core'
import type {
  ApiClient,
  VideoToSpriteUploadResult,
} from './api-client'
import type {
  AnimPreviewOptions,
  AnimPreviewResponse,
  AtlasPackOptions,
  AtlasPackResponse,
  AudioOptions,
  AudioResponse,
  BgRemoveOptions,
  HealthResponse,
  NineSliceOptions,
  NineSliceResponse,
  OptimizeOptions,
  OptimizeResponse,
  Preset,
  ProgressCallback,
  ProgressEvent,
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
  BatchResponse,
} from '~/types/pixiekit'

export function createTauriApiClient(): ApiClient {
  const createProgressChannel = (onProgress?: ProgressCallback) => {
    if (!onProgress) return undefined
    const channel = new Channel<ProgressEvent>()
    channel.onmessage = (event) => {
      onProgress(event)
    }
    return channel
  }

  return {
    async health(): Promise<HealthResponse> {
      return { status: 'ok', version: '2.0.0-tauri' }
    },

    async processBgRemove(input, output, options, onProgress) {
      return invoke<BatchResponse>('run_bg_remove', {
        input,
        output,
        options,
        onProgress: createProgressChannel(onProgress),
      })
    },

    async processBgRemoveUpload() {
      throw new Error('Upload mode not supported in Tauri. Use local paths.')
    },

    async processVectorize(input, output, options, onProgress) {
      return invoke<VectorizeResponse>('run_vectorize', {
        input,
        output,
        options,
        onProgress: createProgressChannel(onProgress),
      })
    },

    async processVectorizeUpload() {
      throw new Error('Upload mode not supported in Tauri. Use local paths.')
    },

    async processVideoToSprite(input, output, options, onProgress) {
      return invoke<VideoToSpriteResponse>('run_video_to_sprite', {
        input,
        output,
        options,
        onProgress: createProgressChannel(onProgress),
      })
    },

    async processVideoToSpriteUpload() {
      throw new Error('Upload mode not supported in Tauri. Use local paths.')
    },

    async processAtlasPack(input, output, options) {
      return invoke<AtlasPackResponse>('run_atlas_pack', {
        input,
        output,
        options,
      })
    },

    async processOptimize(input, output, options, onProgress) {
      return invoke<OptimizeResponse>('run_optimize', {
        input,
        output,
        options,
        onProgress: createProgressChannel(onProgress),
      })
    },

    async processScale(input, output, options, onProgress) {
      return invoke<ScaleResponse>('run_scale', {
        input,
        output,
        options,
        onProgress: createProgressChannel(onProgress),
      })
    },

    async processAudio(input, output, options, onProgress) {
      return invoke<AudioResponse>('run_audio', {
        input,
        output,
        options,
        onProgress: createProgressChannel(onProgress),
      })
    },

    async processTrimPad(input, output, options, onProgress) {
      return invoke<TrimPadResponse>('run_trim_pad', {
        input,
        output,
        options,
        onProgress: createProgressChannel(onProgress),
      })
    },

    async processSvgOptimize(input, output, options, onProgress) {
      return invoke<SvgOptimizeResponse>('run_svg_optimize', {
        input,
        output,
        options,
        onProgress: createProgressChannel(onProgress),
      })
    },

    async processNineSlice(input, output, options, onProgress) {
      return invoke<NineSliceResponse>('run_nine_slice', {
        input,
        output,
        options,
        onProgress: createProgressChannel(onProgress),
      })
    },

    async processNineSliceUpload() {
      throw new Error('Upload mode not supported in Tauri. Use local paths.')
    },

    async processAnimPreview(input, output, options, onProgress) {
      return invoke<AnimPreviewResponse>('run_anim_preview', {
        input,
        output,
        options,
        onProgress: createProgressChannel(onProgress),
      })
    },

    async processAnimPreviewUpload() {
      throw new Error('Upload mode not supported in Tauri. Use local paths.')
    },

    async listPresets() {
      return invoke<Preset<unknown>[]>('list_presets')
    },

    async getPreset(name) {
      return invoke<Preset<unknown> | null>('get_preset', { name })
    },

    async savePreset(name, tool, options) {
      return invoke<Preset<unknown>>('save_preset', { name, tool, options })
    },

    async deletePreset(name) {
      return invoke<void>('delete_preset', { name })
    },
  }
}
