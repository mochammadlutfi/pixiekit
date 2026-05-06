<script setup lang="ts">
import { ref, computed } from 'vue'
import { Play, Save } from 'lucide-vue-next'
import {
  DEFAULT_TRIM_PAD_OPTIONS,
  type TrimPadOptions,
} from '~/types/pixiekit'

useHead({ title: 'Trim & Pad — Pixiekit' })

const inputPath = ref('')
const outputPath = ref('')
const options = ref<TrimPadOptions>({ ...DEFAULT_TRIM_PAD_OPTIONS })
const useBgColor = ref(false)
const bgColorHex = ref('#00FF00')

const beforeUrl = ref<string | undefined>(undefined)
const afterUrl = ref<string | undefined>(undefined)

const running = ref(false)
const lines = ref<{ id: number; text: string; status: 'ok' | 'error' | 'info' }[]>([])
const current = ref(0)
const total = ref(0)

const presets = useToolPreset<TrimPadOptions>('trim-pad', [
  { name: 'Default', options: { ...DEFAULT_TRIM_PAD_OPTIONS } },
  {
    name: 'Square 8px pad',
    options: { ...DEFAULT_TRIM_PAD_OPTIONS, padding: 8, keep_square: true },
  },
])
const presetName = ref('')
const presetList = computed(() => presets.list())

function loadPreset(name: string) {
  const p = presets.load(name)
  if (p) {
    options.value = { ...p.options }
    useBgColor.value = !!p.options.bg_color
    if (p.options.bg_color) bgColorHex.value = p.options.bg_color
  }
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

  const opts: TrimPadOptions = {
    ...options.value,
    bg_color: useBgColor.value ? bgColorHex.value : null,
  }

  let id = 0
  lines.value.push({ id: id++, text: `Starting trim+pad via ${api.mode} API...`, status: 'info' })

  try {
    const res = await api.client.processTrimPad(
      inputPath.value,
      outputPath.value,
      opts,
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
    lines.value.push({
      id: id++,
      text: `Done — ${res.processed} trimmed, ${res.failed} failed in ${res.duration_ms}ms`,
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
    title="Trim & Pad"
    description="Auto-crop transparent (or solid-color) borders, optionally pad uniform px and force square."
  />
  <main class="container py-6 grid gap-6 lg:grid-cols-[1fr_22rem]">
    <div class="space-y-6">
      <SettingsPanel title="Input / Output">
        <PathInput
          v-model="inputPath"
          label="Input folder or file"
          storage-key="trim-pad:input"
        />
        <PathInput
          v-model="outputPath"
          label="Output folder"
          storage-key="trim-pad:output"
        />
        <div>
          <p class="text-sm font-medium mb-1.5">Or drop a file for preview</p>
          <FileDropZone @files="handleFiles" />
        </div>
      </SettingsPanel>

      <SettingsPanel title="Trim settings">
        <div class="grid gap-4 sm:grid-cols-2">
          <div class="space-y-1.5">
            <label class="text-sm font-medium flex items-center justify-between">
              Alpha threshold
              <span class="font-mono text-xs text-muted-foreground">{{ options.alpha_threshold }}</span>
            </label>
            <input
              v-model.number="options.alpha_threshold"
              type="range"
              min="0"
              max="255"
              step="1"
              class="w-full"
            />
          </div>
          <div class="space-y-1.5">
            <label class="text-sm font-medium flex items-center justify-between">
              Padding
              <span class="font-mono text-xs text-muted-foreground">{{ options.padding }}px</span>
            </label>
            <input
              v-model.number="options.padding"
              type="range"
              min="0"
              max="256"
              step="1"
              class="w-full"
            />
          </div>
          <div class="sm:col-span-2 flex items-center gap-2">
            <input
              id="trim-square"
              v-model="options.keep_square"
              type="checkbox"
              class="size-4 rounded border-input"
            />
            <label for="trim-square" class="text-sm">Pad shorter dim to make a square</label>
          </div>
          <div class="sm:col-span-2 flex items-center gap-2">
            <input
              id="trim-bg"
              v-model="useBgColor"
              type="checkbox"
              class="size-4 rounded border-input"
            />
            <label for="trim-bg" class="text-sm">Trim by solid colour instead of alpha</label>
          </div>
          <template v-if="useBgColor">
            <div class="space-y-1.5">
              <label class="text-sm font-medium">Background colour</label>
              <input
                v-model="bgColorHex"
                type="color"
                class="h-10 w-full rounded-md border border-input bg-background"
              />
            </div>
            <div class="space-y-1.5">
              <label class="text-sm font-medium flex items-center justify-between">
                Tolerance
                <span class="font-mono text-xs text-muted-foreground">{{ options.bg_tolerance.toFixed(2) }}</span>
              </label>
              <input
                v-model.number="options.bg_tolerance"
                type="range"
                min="0"
                max="1"
                step="0.01"
                class="w-full"
              />
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
        <BeforeAfterPreview :before="beforeUrl" :after="afterUrl" />
      </SettingsPanel>

      <button
        type="button"
        class="inline-flex h-11 w-full items-center justify-center gap-2 rounded-md bg-primary text-sm font-semibold text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
        :disabled="running"
        @click="run"
      >
        <Play class="size-4" />
        {{ running ? 'Trimming...' : 'Trim & Pad' }}
      </button>

      <ProgressLog :lines="lines" :current="current" :total="total" :running="running" />
    </aside>
  </main>
</template>
