<script setup lang="ts">
import { ref } from 'vue'
import { useDropZone, useFileDialog } from '@vueuse/core'
import { Upload } from 'lucide-vue-next'

interface Props {
  accept?: string
  multiple?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  accept: 'image/*',
  multiple: true,
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
</script>

<template>
  <div
    ref="dropRef"
    class="rounded-lg border-2 border-dashed p-8 text-center transition-colors"
    :class="isOverDropZone ? 'border-primary bg-accent' : 'border-input bg-muted/30'"
  >
    <Upload class="mx-auto mb-2 size-8 text-muted-foreground" />
    <p class="text-sm font-medium">Drop files here, or</p>
    <button
      type="button"
      class="mt-2 inline-flex h-9 items-center rounded-md bg-primary px-3 text-sm font-medium text-primary-foreground hover:bg-primary/90"
      @click="open()"
    >
      Browse files
    </button>
    <p class="mt-2 text-xs text-muted-foreground">{{ accept }}</p>
  </div>
</template>
