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
  DEFAULT_BG_REMOVE_OPTIONS,
  type BgRemoveOptions,
} from '~/types/pixiekit'

useHead({ title: 'BG Remove — Pixiekit' })

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

const options = ref<BgRemoveOptions>({ ...DEFAULT_BG_REMOVE_OPTIONS })

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

const presets = useToolPreset<BgRemoveOptions>('bg-remove', [
  { name: 'Domdom default', options: { ...DEFAULT_BG_REMOVE_OPTIONS } },
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

function buildOutputName(input: string, format: BgRemoveOptions['output_format']): string {
  const dot = input.lastIndexOf('.')
  const stem = dot > 0 ? input.slice(0, dot) : input
  return `${stem}.${format}`
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
    text: `Processing ${items.value.length} file via ${api.mode} API...`,
    status: 'info',
  })

  for (const item of items.value) {
    if (item.status === 'done') continue
    item.status = 'running'
    const t0 = performance.now()
    try {
      const blob = await api.client.processBgRemoveUpload(item.file, options.value)
      const outName = buildOutputName(item.file.name, options.value.output_format)
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
    text: `Done — ${doneCount.value} processed, ${errorCount.value} failed`,
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
    eyebrow="Image cleanup"
    title="BG Remove"
    description="Chroma key + despill + alpha erode. Drop image, atur opsi, klik Process — hasil bisa langsung di-download."
  />

  <main class="container py-6 lg:py-8">
    <div class="grid gap-6 lg:grid-cols-[minmax(0,1fr)_22rem]">
      <!-- ─────────── LEFT: Input + Algorithm + Presets ─────────── -->
      <div class="space-y-6">
        <SettingsPanel
          title="Input"
          :subtitle="
            items.length === 0
              ? 'Drag & drop atau pilih image untuk diproses.'
              : `${items.length} file siap diproses`
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
              <div class="relative size-14 shrink-0 overflow-hidden rounded-md border bg-checker">
                <img
                  :src="item.outputUrl ?? item.beforeUrl"
                  :alt="item.file.name"
                  class="absolute inset-0 size-full object-contain"
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
                      processing
                    </span>
                  </template>
                  <template v-else-if="item.status === 'error'">
                    <span aria-hidden="true">•</span>
                    <span
                      class="inline-flex items-center gap-0.5 text-destructive truncate"
                      :title="item.errorMessage"
                    >
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
                  <span class="hidden sm:inline">Save</span>
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

        <SettingsPanel title="Algorithm" subtitle="Atur chroma key dan post-processing.">
          <div class="grid gap-5 sm:grid-cols-2">
            <div class="space-y-2">
              <label
                for="target_color"
                class="flex items-center justify-between text-sm font-medium"
              >
                Target color
                <span class="font-mono text-[11px] uppercase text-muted-foreground">
                  {{ options.target_color }}
                </span>
              </label>
              <div class="flex items-center gap-2">
                <input
                  id="target_color"
                  v-model="options.target_color"
                  type="color"
                  class="h-10 w-12 cursor-pointer rounded-md border border-input bg-background"
                  aria-label="Pick target color"
                />
                <input
                  v-model="options.target_color"
                  type="text"
                  class="h-10 flex-1 rounded-md border border-input bg-background px-3 font-mono text-xs uppercase focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                  pattern="^#[0-9A-Fa-f]{6}$"
                  aria-label="Hex color"
                />
              </div>
            </div>

            <div class="space-y-2">
              <label for="fuzz" class="flex items-center justify-between text-sm font-medium">
                Fuzz tolerance
                <span class="font-mono text-[11px] tabular-nums text-muted-foreground">
                  {{ options.fuzz.toFixed(2) }}
                </span>
              </label>
              <input
                id="fuzz"
                v-model.number="options.fuzz"
                type="range"
                min="0"
                max="1"
                step="0.01"
                class="w-full accent-primary"
              />
              <div class="flex justify-between text-[10px] text-muted-foreground">
                <span>strict</span>
                <span>fuzzy</span>
              </div>
            </div>

            <div class="space-y-2">
              <label for="erode" class="flex items-center justify-between text-sm font-medium">
                Erode iterations
                <span class="font-mono text-[11px] tabular-nums text-muted-foreground">
                  {{ options.erode }}
                </span>
              </label>
              <input
                id="erode"
                v-model.number="options.erode"
                type="range"
                min="0"
                max="5"
                step="1"
                class="w-full accent-primary"
              />
              <div class="flex justify-between text-[10px] text-muted-foreground">
                <span>none</span>
                <span>aggressive</span>
              </div>
            </div>

            <div class="space-y-2">
              <label for="output_format" class="text-sm font-medium">Output format</label>
              <select
                id="output_format"
                v-model="options.output_format"
                class="h-10 w-full rounded-md border border-input bg-background px-3 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              >
                <option value="png">PNG (lossless)</option>
                <option value="webp">WebP (alpha lossless)</option>
              </select>
            </div>

            <label
              class="group/check flex cursor-pointer items-center gap-2.5 rounded-md border bg-muted/30 px-3 py-2.5 hover:bg-muted/50 transition-colors sm:col-span-1"
            >
              <input
                v-model="options.despill"
                type="checkbox"
                class="size-4 rounded border-input accent-primary"
              />
              <span class="flex-1">
                <span class="block text-sm font-medium">Despill green channel</span>
                <span class="block text-[11px] text-muted-foreground">
                  Buang ghosting hijau di edge subjek.
                </span>
              </span>
            </label>

            <div v-if="options.output_format === 'webp'" class="space-y-2 animate-fade-in">
              <label for="webp_quality" class="flex items-center justify-between text-sm font-medium">
                WebP quality
                <span class="font-mono text-[11px] tabular-nums text-muted-foreground">
                  {{ options.webp_quality }}
                </span>
              </label>
              <input
                id="webp_quality"
                v-model.number="options.webp_quality"
                type="range"
                min="0"
                max="100"
                step="1"
                class="w-full accent-primary"
              />
            </div>
          </div>
        </SettingsPanel>

        <SettingsPanel title="Presets" subtitle="Simpan kombinasi opsi favoritmu.">
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

      <!-- ─────────── RIGHT: Sticky sidebar ─────────── -->
      <aside class="space-y-5 lg:sticky lg:top-20 lg:max-h-[calc(100vh-6rem)] lg:overflow-y-auto lg:pr-1 scroll-thin">
        <SettingsPanel title="Preview" :flush="false">
          <BeforeAfterPreview
            :before="activeItem?.beforeUrl"
            :after="activeItem?.outputUrl"
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
                  ? `Processing ${current}/${total}…`
                  : items.length === 0
                    ? 'Process'
                    : `Process ${items.length} file${items.length === 1 ? '' : 's'}`
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
