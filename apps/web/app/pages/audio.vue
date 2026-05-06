<script setup lang="ts">
import { ref, computed } from 'vue'
import { Play, Save } from 'lucide-vue-next'
import {
  DEFAULT_AUDIO_OPTIONS,
  type AudioOptions,
} from '~/types/pixiekit'

useHead({ title: 'Audio Processor — Pixiekit' })

const inputPath = ref('')
const outputPath = ref('')
const options = ref<AudioOptions>({ ...DEFAULT_AUDIO_OPTIONS })

const result = ref<{
  duration_ms_in?: number
  duration_ms_out?: number
  integrated_lufs?: number | null
  processed: number
  failed: number
} | undefined>(undefined)

const running = ref(false)
const lines = ref<{ id: number; text: string; status: 'ok' | 'error' | 'info' }[]>([])
const current = ref(0)
const total = ref(0)

const presets = useToolPreset<AudioOptions>('audio', [
  { name: 'Game SFX', options: { ...DEFAULT_AUDIO_OPTIONS } },
  {
    name: 'Voice -19 LUFS',
    options: { ...DEFAULT_AUDIO_OPTIONS, target_lufs: -19, channels: 'mono' },
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
  result.value = undefined

  let id = 0
  lines.value.push({
    id: id++,
    text: `Starting audio processor via ${api.mode} API...`,
    status: 'info',
  })

  try {
    const res = await api.client.processAudio(
      inputPath.value,
      outputPath.value,
      options.value,
      e => {
        total.value = e.total
        current.value = e.index
        lines.value.push({
          id: id++,
          text: `[${e.index}/${e.total}] ${e.current_file}`,
          status: 'ok',
        })
      },
    )
    result.value = {
      duration_ms_in: res.duration_ms_in,
      duration_ms_out: res.duration_ms_out,
      integrated_lufs: res.integrated_lufs,
      processed: res.processed,
      failed: res.failed,
    }
    lines.value.push({
      id: id++,
      text: `Done — ${res.processed} ok, ${res.failed} failed in ${res.duration_ms}ms`,
      status: res.failed === 0 ? 'ok' : 'error',
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
    title="Audio Processor"
    description="LUFS normalize + silence trim + format convert via ffmpeg. Batch a folder of WAV/MP3/OGG/FLAC/M4A/Opus."
  />
  <main class="container py-6 grid gap-6 lg:grid-cols-[1fr_22rem]">
    <div class="space-y-6">
      <SettingsPanel title="Input / Output">
        <PathInput
          v-model="inputPath"
          label="Input file or folder"
          storage-key="audio:input"
          placeholder="/path/to/audio.wav or /path/to/folder"
        />
        <PathInput
          v-model="outputPath"
          label="Output folder"
          storage-key="audio:output"
        />
      </SettingsPanel>

      <SettingsPanel title="Format">
        <div class="grid gap-4 sm:grid-cols-2">
          <div class="space-y-1.5">
            <label class="text-sm font-medium">Target format</label>
            <select
              v-model="options.target_format"
              class="h-10 w-full rounded-md border border-input bg-background px-3 text-sm"
            >
              <option value="ogg">OGG (Vorbis)</option>
              <option value="opus">OPUS</option>
              <option value="mp3">MP3</option>
              <option value="wav">WAV (PCM 16-bit)</option>
            </select>
          </div>
          <div class="space-y-1.5">
            <label class="text-sm font-medium">Channels</label>
            <select
              v-model="options.channels"
              class="h-10 w-full rounded-md border border-input bg-background px-3 text-sm"
            >
              <option value="keep">Keep source</option>
              <option value="mono">Mono</option>
              <option value="stereo">Stereo</option>
            </select>
          </div>
          <div class="space-y-1.5">
            <label class="text-sm font-medium flex items-center justify-between">
              Sample rate
              <span class="font-mono text-xs text-muted-foreground">{{ options.sample_rate }} Hz</span>
            </label>
            <select
              v-model.number="options.sample_rate"
              class="h-10 w-full rounded-md border border-input bg-background px-3 text-sm"
            >
              <option :value="22050">22 050 Hz</option>
              <option :value="44100">44 100 Hz</option>
              <option :value="48000">48 000 Hz</option>
              <option :value="96000">96 000 Hz</option>
            </select>
          </div>
          <div v-if="options.target_format !== 'wav'" class="space-y-1.5">
            <label class="text-sm font-medium flex items-center justify-between">
              Bitrate
              <span class="font-mono text-xs text-muted-foreground">{{ options.bitrate_kbps }} kbps</span>
            </label>
            <input
              v-model.number="options.bitrate_kbps"
              type="range"
              min="32"
              max="320"
              step="8"
              class="w-full"
            />
          </div>
        </div>
      </SettingsPanel>

      <SettingsPanel title="Loudness">
        <div class="flex items-center gap-2">
          <input
            id="audio-normalize"
            v-model="options.normalize"
            type="checkbox"
            class="size-4 rounded border-input"
          />
          <label for="audio-normalize" class="text-sm">Normalize loudness (loudnorm)</label>
        </div>
        <div v-if="options.normalize" class="space-y-1.5">
          <label class="text-sm font-medium flex items-center justify-between">
            Target LUFS
            <span class="font-mono text-xs text-muted-foreground">{{ options.target_lufs.toFixed(1) }} LUFS</span>
          </label>
          <input
            v-model.number="options.target_lufs"
            type="range"
            min="-30"
            max="-6"
            step="0.5"
            class="w-full"
          />
        </div>
      </SettingsPanel>

      <SettingsPanel title="Silence">
        <div class="flex items-center gap-2">
          <input
            id="audio-trim-silence"
            v-model="options.trim_silence"
            type="checkbox"
            class="size-4 rounded border-input"
          />
          <label for="audio-trim-silence" class="text-sm">Trim leading/trailing silence</label>
        </div>
        <div v-if="options.trim_silence" class="space-y-1.5">
          <label class="text-sm font-medium flex items-center justify-between">
            Silence threshold
            <span class="font-mono text-xs text-muted-foreground">{{ options.silence_threshold_db.toFixed(0) }} dB</span>
          </label>
          <input
            v-model.number="options.silence_threshold_db"
            type="range"
            min="-80"
            max="-20"
            step="1"
            class="w-full"
          />
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
        <dl v-if="result" class="grid grid-cols-3 text-center text-xs">
          <div>
            <dt class="text-muted-foreground">Duration in</dt>
            <dd class="font-mono">
              {{ result.duration_ms_in !== undefined ? `${(result.duration_ms_in / 1000).toFixed(2)}s` : '—' }}
            </dd>
          </div>
          <div>
            <dt class="text-muted-foreground">Duration out</dt>
            <dd class="font-mono">
              {{ result.duration_ms_out !== undefined ? `${(result.duration_ms_out / 1000).toFixed(2)}s` : '—' }}
            </dd>
          </div>
          <div>
            <dt class="text-muted-foreground">Integrated</dt>
            <dd class="font-mono">
              {{ result.integrated_lufs != null ? `${result.integrated_lufs.toFixed(1)} LUFS` : '—' }}
            </dd>
          </div>
        </dl>
        <p v-else class="text-xs text-muted-foreground">Run the processor to see duration and loudness stats.</p>
      </SettingsPanel>

      <button
        type="button"
        class="inline-flex h-11 w-full items-center justify-center gap-2 rounded-md bg-primary text-sm font-semibold text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
        :disabled="running"
        @click="run"
      >
        <Play class="size-4" />
        {{ running ? 'Processing audio...' : 'Process audio' }}
      </button>

      <ProgressLog :lines="lines" :current="current" :total="total" :running="running" />
    </aside>
  </main>
</template>
