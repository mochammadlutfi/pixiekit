<script setup lang="ts">
import { computed, ref } from 'vue'
import { useDropZone, useFileDialog } from '@vueuse/core'
import { ImagePlus, UploadCloud } from 'lucide-vue-next'

interface Props {
  accept?: string
  multiple?: boolean
  /** Optional helper text shown below the prompt. */
  hint?: string
  /** Display label e.g. "image", "video", "file" — used in the prompt copy. */
  label?: string
}

const props = withDefaults(defineProps<Props>(), {
  accept: 'image/*',
  multiple: true,
  label: '',
})

const promptLabel = computed(() => {
  if (props.label) return props.label
  if (props.accept.startsWith('video/')) return 'video'
  if (props.accept.startsWith('image/')) return 'image'
  return 'file'
})

const emit = defineEmits<{
  files: [files: File[]]
}>()

const dropRef = ref<HTMLDivElement>()

function onDrop(files: File[] | null) {
  if (!files || files.length === 0) return
  emit('files', files)
}

const { isOverDropZone } = useDropZone(dropRef, {
  onDrop,
  dataTypes: props.accept === 'image/*' ? ['image/png', 'image/jpeg', 'image/webp'] : undefined,
})

const { open, onChange } = useFileDialog({
  accept: props.accept,
  multiple: props.multiple,
})

onChange(list => {
  if (!list) return
  emit('files', Array.from(list))
})

const acceptHint = computed(() => {
  if (props.hint) return props.hint
  if (props.accept === 'image/*') return 'PNG, JPG, WEBP — maks 50 MB per file'
  if (props.accept.startsWith('video/')) return 'MP4, MOV, WebM — maks 50 MB per file'
  return `${props.accept} — maks 50 MB per file`
})

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter' || e.key === ' ') {
    e.preventDefault()
    open()
  }
}
</script>

<template>
  <div
    ref="dropRef"
    role="button"
    tabindex="0"
    :aria-label="multiple ? 'Drop files or click to browse' : 'Drop a file or click to browse'"
    class="group relative flex flex-col items-center justify-center gap-3 rounded-xl border-2 border-dashed bg-gradient-to-b from-card to-muted/30 p-8 text-center transition-all duration-200 ease-spring focus:outline-none focus-visible:ring-2 focus-visible:ring-ring sm:p-10"
    :class="
      isOverDropZone
        ? 'border-primary/80 bg-primary/5 shadow-glow scale-[1.01]'
        : 'border-border hover:border-primary/40 hover:bg-accent/30'
    "
    @click="open()"
    @keydown="onKeydown"
  >
    <div
      class="flex size-14 items-center justify-center rounded-full transition-colors duration-200"
      :class="isOverDropZone ? 'bg-primary text-primary-foreground' : 'bg-primary/10 text-primary group-hover:bg-primary/15'"
    >
      <UploadCloud
        v-if="!isOverDropZone"
        class="size-7 transition-transform duration-200"
        aria-hidden="true"
      />
      <ImagePlus
        v-else
        class="size-7 animate-pulse-soft"
        aria-hidden="true"
      />
    </div>

    <div class="space-y-1">
      <p class="text-sm font-semibold sm:text-base">
        {{ isOverDropZone ? 'Lepas untuk menambahkan' : `Drop ${promptLabel} atau klik untuk browse` }}
      </p>
      <p class="text-xs text-muted-foreground">
        {{ acceptHint }}
      </p>
    </div>

    <button
      type="button"
      class="pointer-events-none mt-1 inline-flex h-9 items-center gap-1.5 rounded-md bg-primary px-4 text-sm font-medium text-primary-foreground shadow-sm transition-shadow group-hover:shadow-md"
      tabindex="-1"
    >
      Browse files
    </button>
  </div>
</template>
