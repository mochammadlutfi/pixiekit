<script setup lang="ts">
import { ref, computed } from 'vue'
import { useLocalStorage } from '@vueuse/core'
import { Folder, History } from 'lucide-vue-next'

interface Props {
  modelValue: string
  label: string
  placeholder?: string
  storageKey: string
}

const props = withDefaults(defineProps<Props>(), {
  placeholder: '/Users/you/path/to/folder',
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

async function pickFolder() {
  // File System Access API — Chrome / Edge only
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
    // Browser API gives us a handle but no full filesystem path for security.
    // We fall back to the handle name and let the user verify/edit.
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
        title="Pick folder (Chrome/Edge)"
        @click="pickFolder"
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
