<script setup lang="ts">
import { ref, computed } from 'vue'
import { Play, Save } from 'lucide-vue-next'
import {
  DEFAULT_ATLAS_PACK_OPTIONS,
  type AtlasPackOptions,
} from '~/types/pixiekit'

useHead({ title: 'Atlas Pack — Pixiekit' })

const inputPath = ref('')
const outputPath = ref('')
const options = ref<AtlasPackOptions>({ ...DEFAULT_ATLAS_PACK_OPTIONS })

const result = ref<{
  atlas: string | null
  metadata: string | null
  packed: number
  total: number
  width: number
  height: number
  efficiency: number
} | null>(null)

const running = ref(false)
const lines = ref<{ id: number; text: string; status: 'ok' | 'error' | 'info' }[]>([])

const presets = useToolPreset<AtlasPackOptions>('atlas-pack', [
  { name: 'Default', options: { ...DEFAULT_ATLAS_PACK_OPTIONS } },
  {
    name: 'Mobile-friendly POT',
    options: { ...DEFAULT_ATLAS_PACK_OPTIONS, max_size: 1024, padding: 2, extrude: 2 },
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
  result.value = null

  let id = 0
  lines.value.push({
    id: id++,
    text: `Starting atlas-pack via ${api.mode} API...`,
    status: 'info',
  })

  try {
    const res = await api.client.processAtlasPack(
      inputPath.value,
      outputPath.value,
      options.value,
    )
    result.value = {
      atlas: res.atlas_path,
      metadata: res.metadata_path,
      packed: res.packed,
      total: res.total,
      width: res.atlas_size.w,
      height: res.atlas_size.h,
      efficiency: res.efficiency,
    }
    const failed = res.total - res.packed
    lines.value.push({
      id: id++,
      text: `Packed ${res.packed}/${res.total} into ${res.atlas_size.w}×${res.atlas_size.h} (${Math.round(res.efficiency * 100)}% efficiency) in ${res.duration_ms}ms`,
      status: failed > 0 ? 'error' : 'ok',
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
    title="Atlas Pack"
    description="Pack PNG sprites into a single texture atlas plus Flame-compatible JSON metadata."
  />
  <main class="container py-6 grid gap-6 lg:grid-cols-[1fr_22rem]">
    <div class="space-y-6">
      <SettingsPanel title="Input / Output">
        <PathInput
          v-model="inputPath"
          label="Input folder of PNG sprites"
          storage-key="atlas-pack:input"
        />
        <PathInput
          v-model="outputPath"
          label="Output folder"
          storage-key="atlas-pack:output"
        />
      </SettingsPanel>

      <SettingsPanel title="Atlas settings">
        <div class="grid gap-4 sm:grid-cols-2">
          <div class="space-y-1.5 sm:col-span-2">
            <label class="text-sm font-medium">Atlas name</label>
            <input
              v-model="options.name"
              type="text"
              class="h-10 w-full rounded-md border border-input bg-background px-3 text-sm"
              placeholder="atlas"
            />
          </div>
          <div class="space-y-1.5">
            <label class="text-sm font-medium flex items-center justify-between">
              Max size <span class="font-mono text-xs text-muted-foreground">{{ options.max_size }}px</span>
            </label>
            <input
              v-model.number="options.max_size"
              type="range"
              min="256"
              max="8192"
              step="256"
              class="w-full"
            />
          </div>
          <div class="space-y-1.5">
            <label class="text-sm font-medium flex items-center justify-between">
              Padding <span class="font-mono text-xs text-muted-foreground">{{ options.padding }}px</span>
            </label>
            <input
              v-model.number="options.padding"
              type="range"
              min="0"
              max="16"
              step="1"
              class="w-full"
            />
          </div>
          <div class="space-y-1.5">
            <label class="text-sm font-medium flex items-center justify-between">
              Extrude <span class="font-mono text-xs text-muted-foreground">{{ options.extrude }}px</span>
            </label>
            <input
              v-model.number="options.extrude"
              type="range"
              min="0"
              max="4"
              step="1"
              class="w-full"
            />
          </div>
          <div class="space-y-1.5">
            <label class="text-sm font-medium">Format</label>
            <select
              v-model="options.format"
              class="h-10 w-full rounded-md border border-input bg-background px-3 text-sm"
            >
              <option value="png">PNG (lossless)</option>
              <option value="webp">WebP</option>
            </select>
          </div>
          <div class="flex items-center gap-2">
            <input
              id="pot"
              v-model="options.power_of_two"
              type="checkbox"
              class="size-4 rounded border-input"
            />
            <label for="pot" class="text-sm">Power-of-two atlas (mobile GPU)</label>
          </div>
          <div class="flex items-center gap-2">
            <input
              id="trim"
              v-model="options.trim"
              type="checkbox"
              class="size-4 rounded border-input"
            />
            <label for="trim" class="text-sm">Auto-trim transparent borders</label>
          </div>
          <div v-if="options.format === 'webp'" class="space-y-1.5 sm:col-span-2">
            <label class="text-sm font-medium flex items-center justify-between">
              WebP quality <span class="font-mono text-xs text-muted-foreground">{{ options.webp_quality }}</span>
            </label>
            <input
              v-model.number="options.webp_quality"
              type="range"
              min="0"
              max="100"
              step="1"
              class="w-full"
            />
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
      <SettingsPanel title="Result">
        <div v-if="result" class="space-y-2 text-sm">
          <div class="grid grid-cols-2 gap-2">
            <div class="rounded-md border bg-muted/40 p-2">
              <div class="text-xs text-muted-foreground">Packed</div>
              <div class="font-mono text-base">{{ result.packed }} / {{ result.total }}</div>
            </div>
            <div class="rounded-md border bg-muted/40 p-2">
              <div class="text-xs text-muted-foreground">Efficiency</div>
              <div class="font-mono text-base">{{ Math.round(result.efficiency * 100) }}%</div>
            </div>
            <div class="rounded-md border bg-muted/40 p-2 col-span-2">
              <div class="text-xs text-muted-foreground">Atlas size</div>
              <div class="font-mono text-base">{{ result.width }} × {{ result.height }}</div>
            </div>
          </div>
          <p v-if="result.atlas" class="break-all text-xs text-muted-foreground">
            atlas: <code>{{ result.atlas }}</code>
          </p>
          <p v-if="result.metadata" class="break-all text-xs text-muted-foreground">
            metadata: <code>{{ result.metadata }}</code>
          </p>
        </div>
        <p v-else class="text-sm text-muted-foreground">Run a pack to see results.</p>
      </SettingsPanel>

      <button
        type="button"
        class="inline-flex h-11 w-full items-center justify-center gap-2 rounded-md bg-primary text-sm font-semibold text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
        :disabled="running"
        @click="run"
      >
        <Play class="size-4" />
        {{ running ? 'Packing...' : 'Pack atlas' }}
      </button>

      <ProgressLog
        :lines="lines"
        :current="result ? result.packed : 0"
        :total="result ? result.total : 0"
        :running="running"
      />
    </aside>
  </main>
</template>
