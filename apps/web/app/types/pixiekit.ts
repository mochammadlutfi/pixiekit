/**
 * Shared types between frontend and Phase 5a backend.
 * Mirrors the Rust struct shapes from `crates/web-api`.
 */

// --- BG Remove ---

export type OutputFormat = 'png' | 'webp'

export interface BgRemoveOptions {
  target_color: string // hex e.g. "#00FF00"
  fuzz: number // 0..1
  despill: boolean
  erode: number // 0..5
  output_format: OutputFormat
  webp_quality: number // 0..100
}

export const DEFAULT_BG_REMOVE_OPTIONS: BgRemoveOptions = {
  target_color: '#00FF00',
  fuzz: 0.35,
  despill: true,
  erode: 1,
  output_format: 'png',
  webp_quality: 90,
}

// --- Vectorize ---

export type VectorizeMode = 'color' | 'binary'

export interface VectorizeOptions {
  mode: VectorizeMode
  filter_speckle: number // 0..128
  color_precision: number // 1..8
  layer_difference: number // 0..128
  corner_threshold: number // 0..180
  length_threshold: number // 0..10
  splice_threshold: number // 0..180
  path_precision: number // 0..16
  // Simple slider — when set, overrides corner/length/splice
  smoothness?: number // 0..10
}

export const DEFAULT_VECTORIZE_OPTIONS: VectorizeOptions = {
  mode: 'color',
  filter_speckle: 4,
  color_precision: 6,
  layer_difference: 16,
  corner_threshold: 60,
  length_threshold: 4.0,
  splice_threshold: 45,
  path_precision: 8,
  smoothness: 4,
}

// --- Video → Sprite ---

export interface VideoToSpriteOptions {
  fps: number // 1..30
  frame_size: number // 64..1024
  output_format: OutputFormat
  webp_quality: number // 0..100
  chroma_key: boolean
  chroma_target: string // hex
  chroma_fuzz: number // 0..1
  chroma_despill: boolean
  chroma_erode: number // 0..5
}

export const DEFAULT_VIDEO_TO_SPRITE_OPTIONS: VideoToSpriteOptions = {
  fps: 8,
  frame_size: 256,
  output_format: 'webp',
  webp_quality: 90,
  chroma_key: false,
  chroma_target: '#00FF00',
  chroma_fuzz: 0.35,
  chroma_despill: true,
  chroma_erode: 1,
}

// --- API request / response ---

export interface BatchRequest<O> {
  input: string
  output: string
  options: O
}

export interface ProcessedFile {
  input_path: string
  output_path: string
  duration_ms: number
  ok: boolean
  error?: string
}

export interface BatchResponse {
  processed: number
  failed: number
  duration_ms: number
  files: ProcessedFile[]
}

export interface VideoToSpriteResponse extends BatchResponse {
  frame_count?: number
  frame_size?: number
  fps?: number
  sprite_url?: string
}

export interface VectorizeResponse extends BatchResponse {
  /** SVG content as data URI for first file (preview) */
  svg_data_uri?: string
}

export interface HealthResponse {
  status: 'ok' | 'degraded'
  version: string
}

// --- Progress event (for streaming UI) ---

export interface ProgressEvent {
  index: number
  total: number
  current_file: string
  duration_ms: number
}

export type ProgressCallback = (e: ProgressEvent) => void

// --- Tool identifiers ---

export type ToolId = 'bg-remove' | 'vectorize' | 'video-to-sprite'

export interface ToolMeta {
  id: ToolId
  title: string
  description: string
  href: string
}

// --- Preset (localStorage) ---

export interface Preset<O> {
  name: string
  tool: ToolId
  options: O
  created_at: number // epoch ms
}
