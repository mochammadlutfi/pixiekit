<script setup lang="ts">
import { ref, computed } from 'vue'
import { Play, Save } from 'lucide-vue-next'
import {
  DEFAULT_VECTORIZE_OPTIONS,
  type VectorizeOptions,
} from '~/types/pixiekit'

useHead({ title: 'Vectorize — Pixiekit' })

const inputPath = ref('')
const outputPath = ref('')
const advanced = ref(false)
const options = ref<VectorizeOptions>({ ...DEFAULT_VECTORIZE_OPTIONS })

const beforeUrl = ref<string | undefined>(undefined)
const afterUrl = ref<string | undefined>(undefined)

const running = ref(false)
const lines = ref<{ id: number; text: string; status: 'ok' | 'error' | 'info' }[]>([])
const current = ref(0)
const total = ref(0)

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
  lines.value.push({ id: id++, text: `Starting vectorize via ${api.mode} API...`, status: 'info' })

  try {
    const res = await api.client.processVectorize(
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
      text: `Done — ${res.processed} traced, ${res.failed} failed in ${res.duration_ms}ms`,
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
    title="Vectorize"
    description="Trace raster to SVG via vtracer. Smoothness slider for quick wins."
  />
  <main class="container py-6 grid gap-6 lg:grid-cols-[1fr_22rem]">
    <div class="space-y-6">
      <SettingsPanel title="Input / Output">
        <PathInput
          v-model="inputPath"
          label="Input folder or file"
          storage-key="vectorize:input"
        />
        <PathInput
          v-model="outputPath"
          label="Output folder"
          storage-key="vectorize:output"
        />
        <div>
          <p class="text-sm font-medium mb-1.5">Or drop a file for preview</p>
          <FileDropZone @files="handleFiles" />
        </div>
      </SettingsPanel>

      <SettingsPanel title="Trace settings">
        <div class="grid gap-4 sm:grid-cols-2">
          <div class="space-y-1.5">
            <label class="text-sm font-medium">Mode</label>
            <select
              v-model="options.mode"
              class="h-10 w-full rounded-md border border-input bg-background px-3 text-sm"
            >
              <option value="color">Color (preserve palette)</option>
              <option value="binary">Binary (B&amp;W)</option>
            </select>
          </div>
          <div class="space-y-1.5">
            <label class="text-sm font-medium flex items-center justify-between">
              Filter speckle <span class="font-mono text-xs text-muted-foreground">{{ options.filter_speckle }}px²</span>
            </label>
            <input
              v-model.number="options.filter_speckle"
              type="range"
              min="0"
              max="128"
              step="1"
              class="w-full"
            />
          </div>
          <div class="space-y-1.5 sm:col-span-2">
            <label class="text-sm font-medium flex items-center justify-between">
              Smoothness <span class="font-mono text-xs text-muted-foreground">{{ options.smoothness ?? 4 }}</span>
            </label>
            <input
              v-model.number="options.smoothness"
              type="range"
              min="0"
              max="10"
              step="1"
              class="w-full"
            />
            <p class="text-xs text-muted-foreground">
              Maps to corner / length / splice thresholds. Toggle Advanced to fine-tune.
            </p>
          </div>
          <div class="sm:col-span-2 flex items-center gap-2">
            <input
              id="advanced"
              v-model="advanced"
              type="checkbox"
              class="size-4 rounded border-input"
            />
            <label for="advanced" class="text-sm">Advanced parameters</label>
          </div>
          <template v-if="advanced">
            <div class="space-y-1.5">
              <label class="text-sm font-medium flex items-center justify-between">
                Color precision <span class="font-mono text-xs text-muted-foreground">{{ options.color_precision }}</span>
              </label>
              <input v-model.number="options.color_precision" type="range" min="1" max="8" step="1" class="w-full" />
            </div>
            <div class="space-y-1.5">
              <label class="text-sm font-medium flex items-center justify-between">
                Layer difference <span class="font-mono text-xs text-muted-foreground">{{ options.layer_difference }}</span>
              </label>
              <input v-model.number="options.layer_difference" type="range" min="0" max="128" step="1" class="w-full" />
            </div>
            <div class="space-y-1.5">
              <label class="text-sm font-medium flex items-center justify-between">
                Corner threshold <span class="font-mono text-xs text-muted-foreground">{{ options.corner_threshold }}°</span>
              </label>
              <input v-model.number="options.corner_threshold" type="range" min="0" max="180" step="1" class="w-full" />
            </div>
            <div class="space-y-1.5">
              <label class="text-sm font-medium flex items-center justify-between">
                Splice threshold <span class="font-mono text-xs text-muted-foreground">{{ options.splice_threshold }}°</span>
              </label>
              <input v-model.number="options.splice_threshold" type="range" min="0" max="180" step="1" class="w-full" />
            </div>
            <div class="space-y-1.5">
              <label class="text-sm font-medium flex items-center justify-between">
                Length threshold <span class="font-mono text-xs text-muted-foreground">{{ options.length_threshold.toFixed(1) }}px</span>
              </label>
              <input v-model.number="options.length_threshold" type="range" min="0" max="10" step="0.1" class="w-full" />
            </div>
            <div class="space-y-1.5">
              <label class="text-sm font-medium flex items-center justify-between">
                Path precision <span class="font-mono text-xs text-muted-foreground">{{ options.path_precision }}</span>
              </label>
              <input v-model.number="options.path_precision" type="range" min="0" max="16" step="1" class="w-full" />
            </div>
          </template>
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
        {{ running ? 'Tracing...' : 'Trace' }}
      </button>

      <ProgressLog :lines="lines" :current="current" :total="total" :running="running" />
    </aside>
  </main>
</template>
