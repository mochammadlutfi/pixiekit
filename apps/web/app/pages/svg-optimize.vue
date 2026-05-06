<script setup lang="ts">
import { ref, computed } from 'vue'
import { Play, Save } from 'lucide-vue-next'
import {
  DEFAULT_SVG_OPTIMIZE_OPTIONS,
  type SvgOptimizeOptions,
} from '~/types/pixiekit'

useHead({ title: 'SVG Optimize — Pixiekit' })

const inputPath = ref('')
const outputPath = ref('')
const options = ref<SvgOptimizeOptions>({ ...DEFAULT_SVG_OPTIMIZE_OPTIONS })

const beforeUrl = ref<string | undefined>(undefined)
const afterUrl = ref<string | undefined>(undefined)

const running = ref(false)
const lines = ref<{ id: number; text: string; status: 'ok' | 'error' | 'info' }[]>([])
const current = ref(0)
const total = ref(0)

const presets = useToolPreset<SvgOptimizeOptions>('svg-optimize', [
  { name: 'Default', options: { ...DEFAULT_SVG_OPTIMIZE_OPTIONS } },
  {
    name: 'Lossless',
    options: { ...DEFAULT_SVG_OPTIMIZE_OPTIONS, precision: 6, remove_metadata: false },
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

function handleFiles(files: File[]) {
  const first = files[0]
  if (!first) return
  beforeUrl.value = URL.createObjectURL(first)
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
  afterUrl.value = undefined

  let id = 0
  lines.value.push({ id: id++, text: `Starting SVG optimize via ${api.mode} API...`, status: 'info' })

  try {
    const res = await api.client.processSvgOptimize(
      inputPath.value,
      outputPath.value,
      options.value,
      e => {
        total.value = e.total
        current.value = e.index
        lines.value.push({
          id: id++,
          text: `[${e.index}/${e.total}] ${e.current_file} (${e.duration_ms}ms)`,
          status: 'ok',
        })
      },
    )
    if (res.svg_data_uri) afterUrl.value = res.svg_data_uri
    lines.value.push({
      id: id++,
      text: `Done — ${res.processed} optimized, ${res.failed} failed in ${res.duration_ms}ms`,
      status: res.failed > 0 ? 'error' : 'ok',
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
    title="SVG Optimize"
    description="Minify SVG via usvg parse + serialize. Round path coords, strip metadata, drop hidden elements."
  />
  <main class="container py-6 grid gap-6 lg:grid-cols-[1fr_22rem]">
    <div class="space-y-6">
      <SettingsPanel title="Input / Output">
        <PathInput
          v-model="inputPath"
          label="Input folder or file"
          storage-key="svg-optimize:input"
        />
        <PathInput
          v-model="outputPath"
          label="Output folder"
          storage-key="svg-optimize:output"
        />
        <div>
          <p class="text-sm font-medium mb-1.5">Or drop a file for preview</p>
          <FileDropZone @files="handleFiles" />
        </div>
      </SettingsPanel>

      <SettingsPanel title="Optimize settings">
        <div class="grid gap-4 sm:grid-cols-2">
          <div class="space-y-1.5 sm:col-span-2">
            <label class="text-sm font-medium flex items-center justify-between">
              Coordinate precision
              <span class="font-mono text-xs text-muted-foreground">{{ options.precision }}</span>
            </label>
            <input
              v-model.number="options.precision"
              type="range"
              min="0"
              max="8"
              step="1"
              class="w-full"
            />
            <p class="text-xs text-muted-foreground">
              Decimal places for path coords / transforms. Lower = smaller files, less detail.
            </p>
          </div>
          <div class="flex items-center gap-2">
            <input
              id="svg-meta"
              v-model="options.remove_metadata"
              type="checkbox"
              class="size-4 rounded border-input"
            />
            <label for="svg-meta" class="text-sm">Strip &lt;title&gt;, &lt;desc&gt;, comments</label>
          </div>
          <div class="flex items-center gap-2">
            <input
              id="svg-hidden"
              v-model="options.remove_hidden"
              type="checkbox"
              class="size-4 rounded border-input"
            />
            <label for="svg-hidden" class="text-sm">Strip hidden elements</label>
          </div>
          <div class="flex items-center gap-2">
            <input
              id="svg-merge"
              v-model="options.merge_paths"
              type="checkbox"
              class="size-4 rounded border-input"
            />
            <label for="svg-merge" class="text-sm">Run merge / cleanup pass</label>
          </div>
          <div class="flex items-center gap-2">
            <input
              id="svg-pretty"
              v-model="options.pretty"
              type="checkbox"
              class="size-4 rounded border-input"
            />
            <label for="svg-pretty" class="text-sm">Pretty-print output</label>
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
      <SettingsPanel title="Preview">
        <BeforeAfterPreview :before="beforeUrl" :after="afterUrl" :show-checker-on-after="false" />
      </SettingsPanel>

      <button
        type="button"
        class="inline-flex h-11 w-full items-center justify-center gap-2 rounded-md bg-primary text-sm font-semibold text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
        :disabled="running"
        @click="run"
      >
        <Play class="size-4" />
        {{ running ? 'Optimizing...' : 'Optimize' }}
      </button>

      <ProgressLog :lines="lines" :current="current" :total="total" :running="running" />
    </aside>
  </main>
</template>
