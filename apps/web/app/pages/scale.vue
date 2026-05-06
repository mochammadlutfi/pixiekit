<script setup lang="ts">
import { ref, computed } from 'vue'
import { Play, Save } from 'lucide-vue-next'
import {
  DEFAULT_SCALE_OPTIONS,
  type ScaleOptions,
} from '~/types/pixiekit'

useHead({ title: 'Scale — Pixiekit' })

const inputPath = ref('')
const outputPath = ref('')
const options = ref<ScaleOptions>({ ...DEFAULT_SCALE_OPTIONS })

// Edited as a comma string for ergonomic input; serialised back on run.
const targetScalesText = ref(options.value.target_scales.join(', '))

const running = ref(false)
const lines = ref<{ id: number; text: string; status: 'ok' | 'error' | 'info' }[]>([])
const current = ref(0)
const total = ref(0)

const presets = useToolPreset<ScaleOptions>('scale', [
  { name: 'Flutter @1/2/3x', options: { ...DEFAULT_SCALE_OPTIONS } },
])
const presetName = ref('')
const presetList = computed(() => presets.list())

function loadPreset(name: string) {
  const p = presets.load(name)
  if (p) {
    options.value = { ...p.options }
    targetScalesText.value = p.options.target_scales.join(', ')
  }
}

function savePreset() {
  if (presetName.value.trim().length === 0) return
  parseScalesIntoOptions()
  presets.save(presetName.value, options.value)
  presetName.value = ''
}

function parseScalesIntoOptions(): boolean {
  const parts = targetScalesText.value
    .split(',')
    .map(s => s.trim())
    .filter(s => s.length > 0)
  const out: number[] = []
  for (const p of parts) {
    const n = Number(p)
    if (!Number.isFinite(n) || n <= 0) {
      alert(`Invalid scale: ${p}`)
      return false
    }
    out.push(n)
  }
  if (out.length === 0) {
    alert('Provide at least one density.')
    return false
  }
  options.value.target_scales = out
  return true
}

const api = usePixiekitApi()

async function run() {
  if (running.value) return
  if (!inputPath.value || !outputPath.value) {
    alert('Set input and output paths first.')
    return
  }
  if (!parseScalesIntoOptions()) return

  running.value = true
  lines.value = []
  current.value = 0
  total.value = 0

  let id = 0
  lines.value.push({
    id: id++,
    text: `Starting scale via ${api.mode} API...`,
    status: 'info',
  })

  try {
    const res = await api.client.processScale(
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
    const totalVariants = res.files.reduce((acc, f) => acc + f.variants.length, 0)
    lines.value.push({
      id: id++,
      text: `Done — ${res.processed} files into ${totalVariants} variants in ${res.duration_ms}ms`,
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
    title="Scale"
    description="Resample to multiple densities — Flutter @1x/@2x/@3x, iOS @suffix, or nested folders."
  />
  <main class="container py-6 grid gap-6 lg:grid-cols-[1fr_22rem]">
    <div class="space-y-6">
      <SettingsPanel title="Input / Output">
        <PathInput
          v-model="inputPath"
          label="Input folder or file"
          storage-key="scale:input"
        />
        <PathInput
          v-model="outputPath"
          label="Output folder"
          storage-key="scale:output"
        />
      </SettingsPanel>

      <SettingsPanel title="Densities">
        <div class="grid gap-4 sm:grid-cols-2">
          <div class="space-y-1.5">
            <label class="text-sm font-medium">Source density</label>
            <input
              v-model.number="options.base_scale"
              type="number"
              min="0.1"
              step="0.5"
              class="h-10 w-full rounded-md border border-input bg-background px-3 text-sm"
            />
            <p class="text-xs text-muted-foreground">e.g. 4 means artwork is authored at 4x.</p>
          </div>
          <div class="space-y-1.5">
            <label class="text-sm font-medium">Target densities</label>
            <input
              v-model="targetScalesText"
              type="text"
              placeholder="1.0, 1.5, 2.0, 3.0"
              class="h-10 w-full rounded-md border border-input bg-background px-3 text-sm"
            />
          </div>
          <div class="space-y-1.5">
            <label class="text-sm font-medium">Layout</label>
            <select
              v-model="options.naming"
              class="h-10 w-full rounded-md border border-input bg-background px-3 text-sm"
            >
              <option value="flutter">Flutter (1.0x/foo.png)</option>
              <option value="suffix">iOS (foo@2x.png)</option>
              <option value="nested">Nested (1/foo.png)</option>
            </select>
          </div>
          <div class="space-y-1.5">
            <label class="text-sm font-medium">Filter</label>
            <select
              v-model="options.filter"
              class="h-10 w-full rounded-md border border-input bg-background px-3 text-sm"
            >
              <option value="lanczos">Lanczos (sharp)</option>
              <option value="bilinear">Bilinear (smooth)</option>
              <option value="nearest">Nearest (pixel art)</option>
            </select>
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
      <button
        type="button"
        class="inline-flex h-11 w-full items-center justify-center gap-2 rounded-md bg-primary text-sm font-semibold text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
        :disabled="running"
        @click="run"
      >
        <Play class="size-4" />
        {{ running ? 'Scaling...' : 'Scale' }}
      </button>

      <ProgressLog
        :lines="lines"
        :current="current"
        :total="total"
        :running="running"
      />
    </aside>
  </main>
</template>
