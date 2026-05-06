<script setup lang="ts">
import { ref, computed } from 'vue'
import { useLocalStorage } from '@vueuse/core'
import { Folder, History } from 'lucide-vue-next'

interface Props {
  modelValue: string
  label: string
  placeholder?: string
  storageKey: string
  type?: 'folder' | 'file'
  filters?: string[] // For file picker
}

const props = withDefaults(defineProps<Props>(), {
  placeholder: '/Users/you/path/to/folder',
  type: 'folder',
  filters: () => [],
})

const emit = defineEmits<{
  'update:modelValue': [value: string]
}>()

const RECENT_LIMIT = 5
const recent = useLocalStorage<string[]>(`pixiekit:recent:${props.storageKey}`, [])
const showRecent = ref(false)

const localValue = computed({
  get: () => props.modelValue,
  set: v => emit('update:modelValue', v),
})

function commitRecent(p: string) {
  const trimmed = p.trim()
  if (trimmed.length === 0) return
  const without = recent.value.filter(r => r !== trimmed)
  recent.value = [trimmed, ...without].slice(0, RECENT_LIMIT)
}

function pickRecent(p: string) {
  localValue.value = p
  showRecent.value = false
}

async function pickPath() {
  // Check for Tauri environment
  const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
  if (isTauri) {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      const command = props.type === 'file' ? 'pick_file' : 'pick_folder'
      const args = props.type === 'file' ? { filters: props.filters } : {}
      const path = await invoke<string | null>(command, args)
      if (path) {
        localValue.value = path
        commitRecent(path)
      }
    } catch (err) {
      console.error(`Tauri ${props.type} picker failed:`, err)
    }
    return
  }

  // File System Access API — Chrome / Edge only
  if (props.type === 'file') {
    // We could implement showOpenFilePicker here if needed, but the web version
    // mostly relies on <input type="file"> for single files.
    alert('File picking in browser mode is not yet standard. Please paste the path.')
    return
  }

  const win = window as unknown as {
    showDirectoryPicker?: () => Promise<FileSystemDirectoryHandle>
  }
  if (typeof win.showDirectoryPicker !== 'function') {
    alert(
      'Native folder picker requires Chrome/Edge. Paste the path manually (e.g. Cmd+Opt+C in Finder to copy as path).',
    )
    return
  }
  try {
    const handle = await win.showDirectoryPicker()
    const proposed = `/${handle.name}`
    localValue.value = proposed
    commitRecent(proposed)
  } catch {
    // user cancelled
  }
}
</script>

<template>
  <div class="space-y-1.5">
    <label class="text-sm font-medium">{{ label }}</label>
    <div class="flex gap-2">
      <input
        v-model="localValue"
        type="text"
        :placeholder="placeholder"
        class="flex-1 h-10 rounded-md border border-input bg-background px-3 py-2 text-sm font-mono focus:outline-none focus:ring-2 focus:ring-ring"
        @blur="commitRecent(localValue)"
      />
      <button
        type="button"
        class="inline-flex h-10 w-10 items-center justify-center rounded-md border border-input bg-background text-muted-foreground hover:text-foreground"
        :title="type === 'file' ? 'Pick file' : 'Pick folder'"
        @click="pickPath"
      >
        <Folder class="size-4" />
      </button>
      <div class="relative">
        <button
          type="button"
          class="inline-flex h-10 w-10 items-center justify-center rounded-md border border-input bg-background text-muted-foreground hover:text-foreground"
          :disabled="recent.length === 0"
          title="Recent paths"
          @click="showRecent = !showRecent"
        >
          <History class="size-4" />
        </button>
        <div
          v-if="showRecent && recent.length > 0"
          class="absolute right-0 top-12 z-10 w-72 rounded-md border bg-popover shadow-md"
        >
          <ul class="max-h-60 overflow-auto py-1 text-sm">
            <li
              v-for="p in recent"
              :key="p"
              class="cursor-pointer truncate px-3 py-1.5 font-mono text-xs hover:bg-accent"
              @click="pickRecent(p)"
            >
              {{ p }}
            </li>
          </ul>
        </div>
      </div>
    </div>
  </div>
</template>
