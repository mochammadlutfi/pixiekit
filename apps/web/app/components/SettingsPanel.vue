<script setup lang="ts">
import { Settings2 } from 'lucide-vue-next'

interface Props {
  title?: string
}

withDefaults(defineProps<Props>(), {
  title: 'Settings',
})
</script>

<template>
  <section class="rounded-lg border bg-card">
    <header class="flex items-center gap-2 border-b px-4 py-3">
      <Settings2 class="size-4 text-muted-foreground" />
      <h2 class="text-sm font-semibold">{{ title }}</h2>
    </header>
    <div class="space-y-4 p-4">
      <slot />
    </div>
  </section>
</template>
