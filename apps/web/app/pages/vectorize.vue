<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import {
  AlertCircle,
  CheckCircle2,
  Download,
  Loader2,
  Play,
  Save,
  Sparkles,
  Trash2,
} from 'lucide-vue-next'
import {
  DEFAULT_VECTORIZE_OPTIONS,
  type VectorizeOptions,
} from '~/types/pixiekit'

useHead({ title: 'Vectorize — Pixiekit' })

interface ProcessItem {
  id: number
  file: File
  beforeUrl: string
  status: 'idle' | 'running' | 'done' | 'error'
  outputBlob?: Blob
  outputUrl?: string
  outputName?: string
  errorMessage?: string
  durationMs?: number
}

const advanced = ref(false)
const options = ref<VectorizeOptions>({ ...DEFAULT_VECTORIZE_OPTIONS })

const items = ref<ProcessItem[]>([])
const running = ref(false)
const lines = ref<{ id: number; text: string; status: 'ok' | 'error' | 'info' }[]>([])
const total = computed(() => items.value.length)
const current = computed(() =>
  items.value.filter(i => i.status === 'done' || i.status === 'error').length,
)
const doneCount = computed(() => items.value.filter(i => i.status === 'done').length)
const errorCount = computed(() => items.value.filter(i => i.status === 'error').length)

let nextId = 1

const presets = useToolPreset<VectorizeOptions>('vectorize', [
  { name: 'Default', options: { ...DEFAULT_VECTORIZE_OPTIONS } },
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

const activeItem = computed(
  () => items.value.find(i => i.status === 'done') ?? items.value[0],
)

function handleFiles(files: File[]) {
  for (const f of files) {
    items.value.push({
      id: nextId++,
      file: f,
      beforeUrl: URL.createObjectURL(f),
      status: 'idle',
    })
  }
}

function clearAll() {
  for (const it of items.value) {
    URL.revokeObjectURL(it.beforeUrl)
    if (it.outputUrl) URL.revokeObjectURL(it.outputUrl)
  }
  items.value = []
  lines.value = []
}

function removeItem(id: number) {
  const idx = items.value.findIndex(i => i.id === id)
  if (idx < 0) return
  const it = items.value[idx]!
  URL.revokeObjectURL(it.beforeUrl)
  if (it.outputUrl) URL.revokeObjectURL(it.outputUrl)
  items.value.splice(idx, 1)
}

const api = usePixiekitApi()

function buildOutputName(input: string): string {
  const dot = input.lastIndexOf('.')
  const stem = dot > 0 ? input.slice(0, dot) : input
  return `${stem}.svg`
}

function triggerDownload(blob: Blob, name: string) {
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = name
  document.body.appendChild(a)
  a.click()
  a.remove()
  setTimeout(() => URL.revokeObjectURL(url), 5_000)
}

function downloadOne(item: ProcessItem) {
  if (!item.outputBlob || !item.outputName) return
  triggerDownload(item.outputBlob, item.outputName)
}

function downloadAll() {
  for (const it of items.value) {
    if (it.status === 'done' && it.outputBlob && it.outputName) {
      triggerDownload(it.outputBlob, it.outputName)
    }
  }
}

async function run() {
  if (running.value) return
  if (items.value.length === 0) return
  running.value = true
  lines.value = []
  let logId = 0
  lines.value.push({
    id: logId++,
    text: `Tracing ${items.value.length} file via ${api.mode} API...`,
    status: 'info',
  })

  for (const item of items.value) {
    if (item.status === 'done') continue
    item.status = 'running'
    const t0 = performance.now()
    try {
      const blob = await api.client.processVectorizeUpload(item.file, options.value)
      const outName = buildOutputName(item.file.name)
      if (item.outputUrl) URL.revokeObjectURL(item.outputUrl)
      item.outputBlob = blob
      item.outputUrl = URL.createObjectURL(blob)
      item.outputName = outName
      item.status = 'done'
      item.durationMs = Math.round(performance.now() - t0)
      lines.value.push({
        id: logId++,
        text: `[${current.value}/${total.value}] ${item.file.name} → ${outName} (${item.durationMs}ms)`,
        status: 'ok',
      })
    } catch (err) {
      item.status = 'error'
      item.errorMessage = (err as Error).message
      item.durationMs = Math.round(performance.now() - t0)
      lines.value.push({
        id: logId++,
        text: `Failed: ${item.file.name} — ${item.errorMessage}`,
        status: 'error',
      })
    }
  }

  lines.value.push({
    id: logId++,
    text: `Done — ${doneCount.value} traced, ${errorCount.value} failed`,
    status: errorCount.value > 0 ? 'error' : 'ok',
  })
  running.value = false
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`
  return `${(n / 1024 / 1024).toFixed(2)} MB`
}

function onKeydown(e: KeyboardEvent) {
  const isMod = e.metaKey || e.ctrlKey
  if (!isMod) return
  if (e.key === 'Enter') {
    e.preventDefault()
    run()
  } else if (e.key.toLowerCase() === 'd' && doneCount.value > 0) {
    e.preventDefault()
    downloadAll()
  }
}

onMounted(() => {
  if (typeof window !== 'undefined') {
    window.addEventListener('keydown', onKeydown)
  }
})
onBeforeUnmount(() => {
  if (typeof window !== 'undefined') {
    window.removeEventListener('keydown', onKeydown)
  }
  for (const it of items.value) {
    URL.revokeObjectURL(it.beforeUrl)
    if (it.outputUrl) URL.revokeObjectURL(it.outputUrl)
  }
})
</script>

<template>
  <ToolHeader
    eyebrow="Raster → vector"
    title="Vectorize"
    description="Trace PNG/JPG ke SVG via vtracer. Upload satu atau banyak file, atur smoothness, download SVG-nya."
  />

  <main class="container py-6 lg:py-8">
    <div class="grid gap-6 lg:grid-cols-[minmax(0,1fr)_22rem]">
      <div class="space-y-6">
        <SettingsPanel
          title="Input"
          :subtitle="
            items.length === 0
              ? 'Drag & drop atau pilih raster image (PNG/JPG/WebP).'
              : `${items.length} file siap di-trace`
          "
        >
          <template v-if="items.length > 0" #actions>
            <button
              type="button"
              class="inline-flex items-center gap-1 rounded-md px-2 py-1 text-xs font-medium text-muted-foreground hover:bg-accent hover:text-foreground transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              @click="clearAll"
            >
              <Trash2 class="size-3" />
              Hapus semua
            </button>
          </template>

          <FileDropZone accept="image/*" :multiple="true" @files="handleFiles" />

          <ul
            v-if="items.length > 0"
            class="-m-1 mt-1 grid gap-2 sm:grid-cols-2"
          >
            <li
              v-for="item in items"
              :key="item.id"
              class="group/item relative flex items-center gap-3 rounded-lg border bg-card p-2 pr-2 transition-shadow hover:shadow-sm animate-slide-up"
            >
              <div class="relative size-14 shrink-0 overflow-hidden rounded-md border bg-muted/30">
                <img
                  :src="item.outputUrl ?? item.beforeUrl"
                  :alt="item.file.name"
                  class="absolute inset-0 size-full object-contain p-0.5"
                  loading="lazy"
                />
                <div
                  v-if="item.status === 'running'"
                  class="absolute inset-0 flex items-center justify-center bg-background/70 backdrop-blur-sm"
                >
                  <Loader2 class="size-4 animate-spin text-primary" />
                </div>
              </div>
              <div class="min-w-0 flex-1">
                <p class="truncate text-sm font-medium" :title="item.file.name">
                  {{ item.file.name }}
                </p>
                <div class="mt-0.5 flex items-center gap-1.5 text-[11px] text-muted-foreground">
                  <span class="tabular-nums">{{ formatBytes(item.file.size) }}</span>
                  <template v-if="item.status === 'done'">
                    <span aria-hidden="true">•</span>
                    <span class="inline-flex items-center gap-0.5 text-success">
                      <CheckCircle2 class="size-3" />
                      {{ item.durationMs }}ms
                    </span>
                  </template>
                  <template v-else-if="item.status === 'running'">
                    <span aria-hidden="true">•</span>
                    <span class="inline-flex items-center gap-0.5 text-primary">
                      <Loader2 class="size-3 animate-spin" />
                      tracing
                    </span>
                  </template>
                  <template v-else-if="item.status === 'error'">
                    <span aria-hidden="true">•</span>
                    <span class="inline-flex items-center gap-0.5 text-destructive truncate" :title="item.errorMessage">
                      <AlertCircle class="size-3" />
                      {{ item.errorMessage }}
                    </span>
                  </template>
                </div>
              </div>
              <div class="flex items-center">
                <button
                  v-if="item.status === 'done'"
                  type="button"
                  class="inline-flex h-8 items-center gap-1 rounded-md bg-primary/10 px-2 text-xs font-medium text-primary hover:bg-primary/15 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring transition-colors"
                  :title="`Download ${item.outputName}`"
                  @click="downloadOne(item)"
                >
                  <Download class="size-3.5" />
                  <span class="hidden sm:inline">SVG</span>
                </button>
                <button
                  type="button"
                  class="ml-1 inline-flex size-8 items-center justify-center rounded-md text-muted-foreground/70 opacity-0 transition-all hover:bg-destructive/10 hover:text-destructive focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring group-hover/item:opacity-100"
                  :title="`Hapus ${item.file.name}`"
                  @click="removeItem(item.id)"
                >
                  <Trash2 class="size-3.5" />
                </button>
              </div>
            </li>
          </ul>
        </SettingsPanel>

        <SettingsPanel title="Trace settings" subtitle="Atur mode, smoothness, dan parameter advanced.">
          <div class="grid gap-5 sm:grid-cols-2">
            <div class="space-y-2">
              <label for="vec_mode" class="text-sm font-medium">Mode</label>
              <select
                id="vec_mode"
                v-model="options.mode"
                class="h-10 w-full rounded-md border border-input bg-background px-3 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              >
                <option value="color">Color (preserve palette)</option>
                <option value="binary">Binary (B&amp;W)</option>
              </select>
            </div>

            <div class="space-y-2">
              <label for="vec_speckle" class="flex items-center justify-between text-sm font-medium">
                Filter speckle
                <span class="font-mono text-[11px] tabular-nums text-muted-foreground">{{ options.filter_speckle }}px²</span>
              </label>
              <input
                id="vec_speckle"
                v-model.number="options.filter_speckle"
                type="range"
                min="0"
                max="128"
                step="1"
                class="w-full accent-primary"
              />
              <div class="flex justify-between text-[10px] text-muted-foreground">
                <span>preserve detail</span>
                <span>cleaner</span>
              </div>
            </div>

            <div class="space-y-2 sm:col-span-2">
              <label for="vec_smooth" class="flex items-center justify-between text-sm font-medium">
                Smoothness
                <span class="font-mono text-[11px] tabular-nums text-muted-foreground">{{ options.smoothness ?? 4 }}</span>
              </label>
              <input
                id="vec_smooth"
                v-model.number="options.smoothness"
                type="range"
                min="0"
                max="10"
                step="1"
                class="w-full accent-primary"
              />
              <p class="text-[11px] text-muted-foreground">
                Maps ke <span class="font-mono">corner / length / splice</span> threshold. Aktifkan Advanced untuk fine-tune.
              </p>
            </div>

            <label
              class="flex cursor-pointer items-center gap-2.5 rounded-md border bg-muted/30 px-3 py-2.5 hover:bg-muted/50 transition-colors sm:col-span-2"
            >
              <input
                v-model="advanced"
                type="checkbox"
                class="size-4 rounded border-input accent-primary"
              />
              <span class="text-sm font-medium">Advanced parameters</span>
              <span class="ml-auto text-[11px] text-muted-foreground">
                override smoothness mapping
              </span>
            </label>

            <template v-if="advanced">
              <div class="space-y-2 animate-fade-in">
                <label for="vec_color_prec" class="flex items-center justify-between text-sm font-medium">
                  Color precision
                  <span class="font-mono text-[11px] tabular-nums text-muted-foreground">{{ options.color_precision }}</span>
                </label>
                <input id="vec_color_prec" v-model.number="options.color_precision" type="range" min="1" max="8" step="1" class="w-full accent-primary" />
              </div>
              <div class="space-y-2 animate-fade-in">
                <label for="vec_layer" class="flex items-center justify-between text-sm font-medium">
                  Layer difference
                  <span class="font-mono text-[11px] tabular-nums text-muted-foreground">{{ options.layer_difference }}</span>
                </label>
                <input id="vec_layer" v-model.number="options.layer_difference" type="range" min="0" max="128" step="1" class="w-full accent-primary" />
              </div>
              <div class="space-y-2 animate-fade-in">
                <label for="vec_corner" class="flex items-center justify-between text-sm font-medium">
                  Corner threshold
                  <span class="font-mono text-[11px] tabular-nums text-muted-foreground">{{ options.corner_threshold }}°</span>
                </label>
                <input id="vec_corner" v-model.number="options.corner_threshold" type="range" min="0" max="180" step="1" class="w-full accent-primary" />
              </div>
              <div class="space-y-2 animate-fade-in">
                <label for="vec_splice" class="flex items-center justify-between text-sm font-medium">
                  Splice threshold
                  <span class="font-mono text-[11px] tabular-nums text-muted-foreground">{{ options.splice_threshold }}°</span>
                </label>
                <input id="vec_splice" v-model.number="options.splice_threshold" type="range" min="0" max="180" step="1" class="w-full accent-primary" />
              </div>
              <div class="space-y-2 animate-fade-in">
                <label for="vec_length" class="flex items-center justify-between text-sm font-medium">
                  Length threshold
                  <span class="font-mono text-[11px] tabular-nums text-muted-foreground">{{ options.length_threshold.toFixed(1) }}px</span>
                </label>
                <input id="vec_length" v-model.number="options.length_threshold" type="range" min="0" max="10" step="0.1" class="w-full accent-primary" />
              </div>
              <div class="space-y-2 animate-fade-in">
                <label for="vec_path_prec" class="flex items-center justify-between text-sm font-medium">
                  Path precision
                  <span class="font-mono text-[11px] tabular-nums text-muted-foreground">{{ options.path_precision }}</span>
                </label>
                <input id="vec_path_prec" v-model.number="options.path_precision" type="range" min="0" max="16" step="1" class="w-full accent-primary" />
              </div>
            </template>
          </div>
        </SettingsPanel>

        <SettingsPanel title="Presets" subtitle="Simpan kombinasi untuk dipakai ulang.">
          <div v-if="presetList.length > 0" class="flex flex-wrap gap-2">
            <button
              v-for="p in presetList"
              :key="p.name"
              type="button"
              class="inline-flex h-8 items-center gap-1.5 rounded-full border bg-card px-3 text-xs font-medium text-foreground/80 hover:border-primary/40 hover:bg-accent hover:text-foreground transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              @click="loadPreset(p.name)"
            >
              <Sparkles class="size-3 text-primary" />
              {{ p.name }}
            </button>
          </div>
          <div class="flex gap-2">
            <input
              v-model="presetName"
              placeholder="Nama preset baru…"
              class="h-9 flex-1 rounded-md border border-input bg-background px-3 text-sm placeholder:text-muted-foreground/70 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              @keydown.enter.prevent="savePreset"
            />
            <button
              type="button"
              class="inline-flex h-9 items-center gap-1.5 rounded-md border bg-card px-3 text-sm font-medium hover:bg-accent transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50"
              :disabled="presetName.trim().length === 0"
              @click="savePreset"
            >
              <Save class="size-3.5" />
              Save
            </button>
          </div>
        </SettingsPanel>
      </div>

      <aside class="space-y-5 lg:sticky lg:top-20 lg:max-h-[calc(100vh-6rem)] lg:overflow-y-auto lg:pr-1 scroll-thin">
        <SettingsPanel title="Preview">
          <BeforeAfterPreview
            :before="activeItem?.beforeUrl"
            :after="activeItem?.outputUrl"
            :show-checker-on-after="false"
          />
          <p
            v-if="activeItem?.outputName"
            class="mt-3 truncate rounded-md bg-muted/50 px-2.5 py-1.5 text-center font-mono text-[11px] text-muted-foreground"
            :title="activeItem.outputName"
          >
            ↓ {{ activeItem.outputName }}
          </p>
        </SettingsPanel>

        <div class="space-y-2">
          <button
            type="button"
            class="relative inline-flex h-12 w-full items-center justify-center gap-2 overflow-hidden rounded-lg gradient-brand text-sm font-semibold text-primary-foreground shadow-md transition-all hover:shadow-lg active:scale-[0.99] disabled:cursor-not-allowed disabled:opacity-50 disabled:shadow-none focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
            :disabled="running || items.length === 0"
            @click="run"
          >
            <Loader2 v-if="running" class="size-4 animate-spin" />
            <Play v-else class="size-4" />
            <span>
              {{
                running
                  ? `Tracing ${current}/${total}…`
                  : items.length === 0
                    ? 'Trace'
                    : `Trace ${items.length} file${items.length === 1 ? '' : 's'}`
              }}
            </span>
            <kbd
              v-if="!running && items.length > 0"
              class="ml-auto hidden items-center gap-0.5 rounded bg-white/15 px-1.5 py-0.5 text-[10px] font-mono font-medium sm:inline-flex"
            >
              ⌘ ↵
            </kbd>
          </button>

          <button
            v-if="doneCount > 0"
            type="button"
            class="inline-flex h-10 w-full items-center justify-center gap-2 rounded-lg border border-border bg-card text-sm font-medium hover:bg-accent transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
            @click="downloadAll"
          >
            <Download class="size-4" />
            Download all
            <span class="rounded-full bg-primary/10 px-1.5 py-0.5 text-[11px] font-mono text-primary tabular-nums">
              {{ doneCount }}
            </span>
            <kbd class="ml-auto hidden items-center gap-0.5 rounded bg-muted px-1.5 py-0.5 text-[10px] font-mono font-medium sm:inline-flex">
              ⌘ D
            </kbd>
          </button>
        </div>

        <ProgressLog
          :lines="lines"
          :current="current"
          :total="total"
          :running="running"
        />
      </aside>
    </div>
  </main>
</template>
