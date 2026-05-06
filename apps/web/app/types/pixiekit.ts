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

// --- Atlas Pack ---

export interface AtlasPackOptions {
  name: string
  max_size: number // 256..8192
  padding: number // 0..16
  extrude: number // 0..4
  power_of_two: boolean
  trim: boolean
  format: OutputFormat
  webp_quality: number // 0..100
}

export const DEFAULT_ATLAS_PACK_OPTIONS: AtlasPackOptions = {
  name: 'atlas',
  max_size: 2048,
  padding: 2,
  extrude: 1,
  power_of_two: true,
  trim: true,
  format: 'png',
  webp_quality: 90,
}

export interface AtlasPackResponse {
  atlas_path: string | null
  metadata_path: string | null
  packed: number
  total: number
  atlas_size: { w: number; h: number }
  efficiency: number // 0..1
  duration_ms: number
}

// --- Image Optimizer ---

export type OptimizeTargetFormat = 'png' | 'webp' | 'keep'

export interface OptimizeOptions {
  target_format: OptimizeTargetFormat
  quality: number // 0..100
  lossless: boolean
  strip_metadata: boolean
  optimization_level: number // 0..6
}

export const DEFAULT_OPTIMIZE_OPTIONS: OptimizeOptions = {
  target_format: 'webp',
  quality: 90,
  lossless: false,
  strip_metadata: true,
  optimization_level: 3,
}

// --- Multi-Resolution Scaler ---

export type ScaleNamingMode = 'flutter' | 'suffix' | 'nested'
export type ScaleFilter = 'lanczos' | 'bilinear' | 'nearest'

export interface ScaleOptions {
  base_scale: number
  target_scales: number[]
  naming: ScaleNamingMode
  filter: ScaleFilter
}

export const DEFAULT_SCALE_OPTIONS: ScaleOptions = {
  base_scale: 4.0,
  target_scales: [1.0, 1.5, 2.0, 3.0],
  naming: 'flutter',
  filter: 'lanczos',
}

// --- Audio Processor ---

export type AudioTargetFormat = 'ogg' | 'opus' | 'mp3' | 'wav'
export type AudioChannels = 'mono' | 'stereo' | 'keep'

export interface AudioOptions {
  target_format: AudioTargetFormat
  target_lufs: number // typical -19..-14
  normalize: boolean
  trim_silence: boolean
  silence_threshold_db: number // typical -60..-30
  sample_rate: number // 8000..192000
  channels: AudioChannels
  bitrate_kbps: number // 32..320 (ignored for wav)
}

export const DEFAULT_AUDIO_OPTIONS: AudioOptions = {
  target_format: 'ogg',
  target_lufs: -16.0,
  normalize: true,
  trim_silence: true,
  silence_threshold_db: -50.0,
  sample_rate: 44100,
  channels: 'keep',
  bitrate_kbps: 128,
}

// --- Trim & Pad ---

export interface TrimPadOptions {
  alpha_threshold: number // 0..255
  padding: number // 0..N px
  keep_square: boolean
  /** Hex e.g. "#00FF00" — null/undefined means trim by alpha. */
  bg_color?: string | null
  bg_tolerance: number // 0..1
}

export const DEFAULT_TRIM_PAD_OPTIONS: TrimPadOptions = {
  alpha_threshold: 1,
  padding: 0,
  keep_square: false,
  bg_color: null,
  bg_tolerance: 0.05,
}

// --- SVG Optimize ---

export interface SvgOptimizeOptions {
  precision: number // 0..8
  remove_metadata: boolean
  remove_hidden: boolean
  merge_paths: boolean
  pretty: boolean
}

export const DEFAULT_SVG_OPTIMIZE_OPTIONS: SvgOptimizeOptions = {
  precision: 3,
  remove_metadata: true,
  remove_hidden: true,
  merge_paths: true,
  pretty: false,
}

// --- Nine Slice ---

export type NineSliceMode = 'stretch' | 'repeat' | 'tile' | 'metadata'

export interface NineSliceOptions {
  top: number
  right: number
  bottom: number
  left: number
  mode: NineSliceMode
}

export const DEFAULT_NINE_SLICE_OPTIONS: NineSliceOptions = {
  top: 0,
  right: 0,
  bottom: 0,
  left: 0,
  mode: 'metadata',
}

// --- Anim Preview ---

export type AnimPreviewFormat = 'gif' | 'mp4' | 'webm'

export interface AnimPreviewOptions {
  fps: number
  format: AnimPreviewFormat
  loop_anim: boolean
  upscale: number
  frame_size?: number
}

export const DEFAULT_ANIM_PREVIEW_OPTIONS: AnimPreviewOptions = {
  fps: 8,
  format: 'gif',
  loop_anim: true,
  upscale: 1,
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

export interface OptimizedFile extends ProcessedFile {
  input_size?: number
  output_size?: number
  ratio?: number
}

export interface OptimizeResponse {
  processed: number
  failed: number
  duration_ms: number
  files: OptimizedFile[]
}

export interface ScaledFile {
  input: string
  variants: string[]
  status: 'ok' | 'failed'
  error?: string
}

export interface ScaleResponse {
  processed: number
  failed: number
  duration_ms: number
  files: ScaledFile[]
}

export interface AudioFileEntry extends ProcessedFile {
  duration_ms_in?: number
  duration_ms_out?: number
  integrated_lufs?: number | null
}

export interface AudioResponse extends BatchResponse {
  /** Aggregated stats for the first / single processed file (for preview UI). */
  duration_ms_in?: number
  duration_ms_out?: number
  integrated_lufs?: number | null
}

export interface TrimPadFile extends ProcessedFile {
  output_size?: [number, number]
  bbox?: [number, number, number, number]
}

export interface TrimPadResponse extends BatchResponse {
  files: TrimPadFile[]
}

export interface SvgOptimizeFile extends ProcessedFile {
  input_size?: number
  output_size?: number
  ratio?: number
}

export interface SvgOptimizeResponse extends BatchResponse {
  files: SvgOptimizeFile[]
  /** SVG content as data URI for first file (preview) */
  svg_data_uri?: string
}

export interface NineSliceFile extends ProcessedFile {
  outputs: string[]
}

export interface NineSliceResponse extends BatchResponse {
  files: NineSliceFile[]
}

export interface AnimPreviewFile extends ProcessedFile {
  output?: string
}

export interface AnimPreviewResponse extends BatchResponse {
  files: AnimPreviewFile[]
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

export type ToolId =
  | 'bg-remove'
  | 'vectorize'
  | 'video-to-sprite'
  | 'atlas-pack'
  | 'optimize'
  | 'scale'
  | 'audio'
  | 'trim-pad'
  | 'svg-optimize'
  | 'nine-slice'
  | 'anim-preview'

export interface ToolMeta {
  id: ToolId
  title: string
  description: string
  href: string
}

// --- Preset ---
//
// In real mode, presets are stored on the backend as JSON files under
// `~/.config/pixiekit/presets/<name>.json` (PRD §9.1) and exposed via
// `/api/presets`. In mock mode, the same shape is mirrored in localStorage so
// the frontend can run standalone. Preset names are flat across tools — the
// `tool` field discriminates ownership; collisions across tools overwrite.

export interface Preset<O> {
  name: string
  tool: ToolId
  options: O
  /** Optional: present in mock mode (epoch ms). Real backend doesn't track it. */
  created_at?: number
}

export interface PresetSavePayload<O> {
  tool: ToolId
  options: O
}
