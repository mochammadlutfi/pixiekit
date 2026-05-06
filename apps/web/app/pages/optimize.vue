<script setup lang="ts">
import { ref, computed } from 'vue'
import { Play, Save } from 'lucide-vue-next'
import {
  DEFAULT_OPTIMIZE_OPTIONS,
  type OptimizeOptions,
} from '~/types/pixiekit'

useHead({ title: 'Optimize — Pixiekit' })

const inputPath = ref('')
const outputPath = ref('')
const options = ref<OptimizeOptions>({ ...DEFAULT_OPTIMIZE_OPTIONS })

const running = ref(false)
const lines = ref<{ id: number; text: string; status: 'ok' | 'error' | 'info' }[]>([])
const current = ref(0)
const total = ref(0)
const totalSaved = ref<number | null>(null)

const presets = useToolPreset<OptimizeOptions>('optimize', [
  { name: 'Web default', options: { ...DEFAULT_OPTIMIZE_OPTIONS } },
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
  totalSaved.value = null

  let id = 0
  lines.value.push({
    id: id++,
    text: `Starting optimize via ${api.mode} API...`,
    status: 'info',
  })

  try {
    const res = await api.client.processOptimize(
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
    const saved = res.files.reduce((acc, f) => {
      const inp = f.input_size ?? 0
      const outp = f.output_size ?? 0
      return acc + (inp - outp)
    }, 0)
    totalSaved.value = saved
    lines.value.push({
      id: id++,
      text: `Done — ${res.processed} optimized, ${res.failed} failed in ${res.duration_ms}ms (saved ${formatBytes(saved)})`,
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

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`
  return `${(n / 1024 / 1024).toFixed(2)} MB`
}
</script>

<template>
  <ToolHeader
    title="Optimize"
    description="Shrink PNG/JPG/WebP — oxipng for PNG, lossy/lossless re-encode for the rest."
  />
  <main class="container py-6 grid gap-6 lg:grid-cols-[1fr_22rem]">
    <div class="space-y-6">
      <SettingsPanel title="Input / Output">
        <PathInput
          v-model="inputPath"
          label="Input folder or file"
          storage-key="optimize:input"
        />
        <PathInput
          v-model="outputPath"
          label="Output folder"
          storage-key="optimize:output"
        />
      </SettingsPanel>

      <SettingsPanel title="Encoding">
        <div class="grid gap-4 sm:grid-cols-2">
          <div class="space-y-1.5">
            <label class="text-sm font-medium">Target format</label>
            <select
              v-model="options.target_format"
              class="h-10 w-full rounded-md border border-input bg-background px-3 text-sm"
            >
              <option value="webp">WebP</option>
              <option value="png">PNG</option>
              <option value="keep">Keep input format</option>
            </select>
          </div>
          <div class="space-y-1.5">
            <label class="text-sm font-medium flex items-center justify-between">
              Quality <span class="font-mono text-xs text-muted-foreground">{{ options.quality }}</span>
            </label>
            <input
              v-model.number="options.quality"
              type="range"
              min="0"
              max="100"
              step="1"
              class="w-full"
              :disabled="options.lossless"
            />
          </div>
          <div class="space-y-1.5">
            <label class="text-sm font-medium flex items-center justify-between">
              oxipng level <span class="font-mono text-xs text-muted-foreground">{{ options.optimization_level }}</span>
            </label>
            <input
              v-model.number="options.optimization_level"
              type="range"
              min="0"
              max="6"
              step="1"
              class="w-full"
            />
            <p class="text-xs text-muted-foreground">Higher = smaller PNG, slower.</p>
          </div>
          <div class="flex items-center gap-2 pt-1">
            <input
              id="lossless"
              v-model="options.lossless"
              type="checkbox"
              class="size-4 rounded border-input"
            />
            <label for="lossless" class="text-sm">Lossless WebP</label>
          </div>
          <div class="flex items-center gap-2 pt-1">
            <input
              id="strip"
              v-model="options.strip_metadata"
              type="checkbox"
              class="size-4 rounded border-input"
            />
            <label for="strip" class="text-sm">Strip metadata</label>
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
      <SettingsPanel title="Summary">
        <div class="text-sm text-muted-foreground">
          <p v-if="totalSaved === null">Run to see byte savings.</p>
          <p v-else>
            Total saved:
            <span class="font-mono text-foreground">{{ formatBytes(totalSaved) }}</span>
          </p>
        </div>
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

      <ProgressLog
        :lines="lines"
        :current="current"
        :total="total"
        :running="running"
      />
    </aside>
  </main>
</template>
