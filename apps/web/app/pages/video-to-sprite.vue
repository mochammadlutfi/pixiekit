<script setup lang="ts">
import { ref, computed } from 'vue'
import { Play, Save } from 'lucide-vue-next'
import {
  DEFAULT_VIDEO_TO_SPRITE_OPTIONS,
  type VideoToSpriteOptions,
} from '~/types/pixiekit'

useHead({ title: 'Video → Sprite — Pixiekit' })

const inputPath = ref('')
const outputPath = ref('')
const options = ref<VideoToSpriteOptions>({ ...DEFAULT_VIDEO_TO_SPRITE_OPTIONS })

const spriteUrl = ref<string | undefined>(undefined)
const meta = ref<{ frame_count: number; fps: number; frame_size: number } | undefined>(undefined)

const running = ref(false)
const lines = ref<{ id: number; text: string; status: 'ok' | 'error' | 'info' }[]>([])
const current = ref(0)
const total = ref(0)

const presets = useToolPreset<VideoToSpriteOptions>('video-to-sprite', [
  { name: 'Domdom 8fps', options: { ...DEFAULT_VIDEO_TO_SPRITE_OPTIONS } },
  {
    name: 'High-fidelity 12fps',
    options: { ...DEFAULT_VIDEO_TO_SPRITE_OPTIONS, fps: 12, frame_size: 512 },
  },
])
const presetName = ref('')
const presetList = computed(() => presets.list())

function loadPreset(name: string) {
  const p = presets.load(name)
  if (p) options.value = { ...p.options }
}
function savePreset() {
  if (presetName.value.trim().length === 0) return
  presets.save(presetName.value, options.value)
  presetName.value = ''
}

const api = usePixiekitApi()

async function run() {
  if (running.value) return
  if (!inputPath.value || !outputPath.value) {
    alert('Set input and output paths first.')
    return
  }
  running.value = true
  lines.value = []
  current.value = 0
  total.value = 0
  spriteUrl.value = undefined
  meta.value = undefined

  let id = 0
  lines.value.push({ id: id++, text: `Starting video→sprite via ${api.mode} API...`, status: 'info' })

  try {
    const res = await api.client.processVideoToSprite(
      inputPath.value,
      outputPath.value,
      options.value,
      e => {
        total.value = e.total
        current.value = e.index
        // Don't spam log with 32+ frames — sample every 4th
        if (e.index === 1 || e.index === e.total || e.index % 4 === 0) {
          lines.value.push({
            id: id++,
            text: `[${e.index}/${e.total}] ${e.current_file}`,
            status: 'ok',
          })
        }
      },
    )
    if (res.sprite_url) spriteUrl.value = res.sprite_url
    if (res.frame_count !== undefined) {
      meta.value = {
        frame_count: res.frame_count,
        fps: res.fps ?? options.value.fps,
        frame_size: res.frame_size ?? options.value.frame_size,
      }
    }
    lines.value.push({
      id: id++,
      text: `Done — ${res.frame_count ?? 0} frames @ ${res.fps ?? options.value.fps}fps in ${res.duration_ms}ms`,
      status: 'ok',
    })
  } catch (err) {
    lines.value.push({
      id: id++,
      text: `Failed: ${(err as Error).message}`,
      status: 'error',
    })
  } finally {
    running.value = false
  }
}
</script>

<template>
  <ToolHeader
    title="Video → Sprite"
    description="ffmpeg frame extraction + horizontal stitch. Optional chroma key per frame."
  />
  <main class="container py-6 grid gap-6 lg:grid-cols-[1fr_22rem]">
    <div class="space-y-6">
      <SettingsPanel title="Input / Output">
        <PathInput
          v-model="inputPath"
          label="Input file or folder"
          storage-key="video-to-sprite:input"
          placeholder="/path/to/video.mp4 or /path/to/folder"
        />
        <PathInput
          v-model="outputPath"
          label="Output folder"
          storage-key="video-to-sprite:output"
        />
      </SettingsPanel>

      <SettingsPanel title="Sprite settings">
        <div class="grid gap-4 sm:grid-cols-2">
          <div class="space-y-1.5">
            <label class="text-sm font-medium flex items-center justify-between">
              Target FPS <span class="font-mono text-xs text-muted-foreground">{{ options.fps }}</span>
            </label>
            <input v-model.number="options.fps" type="range" min="1" max="30" step="1" class="w-full" />
          </div>
          <div class="space-y-1.5">
            <label class="text-sm font-medium flex items-center justify-between">
              Frame size <span class="font-mono text-xs text-muted-foreground">{{ options.frame_size }}px</span>
            </label>
            <input v-model.number="options.frame_size" type="range" min="64" max="1024" step="32" class="w-full" />
          </div>
          <div class="space-y-1.5">
            <label class="text-sm font-medium">Output format</label>
            <select
              v-model="options.output_format"
              class="h-10 w-full rounded-md border border-input bg-background px-3 text-sm"
            >
              <option value="webp">WebP (alpha lossless)</option>
              <option value="png">PNG (lossless)</option>
            </select>
          </div>
          <div v-if="options.output_format === 'webp'" class="space-y-1.5">
            <label class="text-sm font-medium flex items-center justify-between">
              WebP quality <span class="font-mono text-xs text-muted-foreground">{{ options.webp_quality }}</span>
            </label>
            <input v-model.number="options.webp_quality" type="range" min="0" max="100" step="1" class="w-full" />
          </div>
        </div>
      </SettingsPanel>

      <SettingsPanel title="Chroma key (optional)">
        <div class="flex items-center gap-2">
          <input
            id="chroma-key"
            v-model="options.chroma_key"
            type="checkbox"
            class="size-4 rounded border-input"
          />
          <label for="chroma-key" class="text-sm">Apply BG remove per frame</label>
        </div>
        <div v-if="options.chroma_key" class="grid gap-4 sm:grid-cols-2">
          <div class="space-y-1.5">
            <label class="text-sm font-medium flex items-center justify-between">
              Target color <span class="font-mono text-xs text-muted-foreground">{{ options.chroma_target }}</span>
            </label>
            <input
              v-model="options.chroma_target"
              type="color"
              class="h-10 w-full rounded-md border border-input bg-background"
            />
          </div>
          <div class="space-y-1.5">
            <label class="text-sm font-medium flex items-center justify-between">
              Fuzz <span class="font-mono text-xs text-muted-foreground">{{ options.chroma_fuzz.toFixed(2) }}</span>
            </label>
            <input v-model.number="options.chroma_fuzz" type="range" min="0" max="1" step="0.01" class="w-full" />
          </div>
          <div class="space-y-1.5">
            <label class="text-sm font-medium flex items-center justify-between">
              Erode <span class="font-mono text-xs text-muted-foreground">{{ options.chroma_erode }}</span>
            </label>
            <input v-model.number="options.chroma_erode" type="range" min="0" max="5" step="1" class="w-full" />
          </div>
          <div class="flex items-center gap-2 pt-1">
            <input
              id="chroma-despill"
              v-model="options.chroma_despill"
              type="checkbox"
              class="size-4 rounded border-input"
            />
            <label for="chroma-despill" class="text-sm">Despill green</label>
          </div>
        </div>
      </SettingsPanel>

      <SettingsPanel title="Presets">
        <div class="flex flex-wrap gap-2">
          <button
            v-for="p in presetList"
            :key="p.name"
            type="button"
            class="inline-flex h-8 items-center rounded-md border px-3 text-xs hover:bg-accent"
            @click="loadPreset(p.name)"
          >
            {{ p.name }}
          </button>
        </div>
        <div class="flex gap-2">
          <input
            v-model="presetName"
            placeholder="Preset name..."
            class="flex-1 h-9 rounded-md border border-input bg-background px-3 text-sm"
          />
          <button
            type="button"
            class="inline-flex h-9 items-center gap-1.5 rounded-md border px-3 text-sm hover:bg-accent"
            @click="savePreset"
          >
            <Save class="size-3.5" />
            Save
          </button>
        </div>
      </SettingsPanel>
    </div>

    <aside class="space-y-6">
      <SettingsPanel title="Sprite preview">
        <div class="rounded-md border bg-checker p-2 flex items-center justify-center min-h-[8rem]">
          <img
            v-if="spriteUrl"
            :src="spriteUrl"
            alt="Sprite sheet preview"
            class="max-h-32 max-w-full object-contain"
          />
          <span v-else class="text-xs text-muted-foreground">No sprite yet</span>
        </div>
        <dl v-if="meta" class="mt-3 grid grid-cols-3 text-center text-xs">
          <div>
            <dt class="text-muted-foreground">Frames</dt>
            <dd class="font-mono">{{ meta.frame_count }}</dd>
          </div>
          <div>
            <dt class="text-muted-foreground">FPS</dt>
            <dd class="font-mono">{{ meta.fps }}</dd>
          </div>
          <div>
            <dt class="text-muted-foreground">Size</dt>
            <dd class="font-mono">{{ meta.frame_size }}px</dd>
          </div>
        </dl>
      </SettingsPanel>

      <button
        type="button"
        class="inline-flex h-11 w-full items-center justify-center gap-2 rounded-md bg-primary text-sm font-semibold text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
        :disabled="running"
        @click="run"
      >
        <Play class="size-4" />
        {{ running ? 'Building sprite...' : 'Build sprite' }}
      </button>

      <ProgressLog :lines="lines" :current="current" :total="total" :running="running" />
    </aside>
  </main>
</template>
