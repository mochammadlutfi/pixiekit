<script setup lang="ts">
import { ChevronLeft } from 'lucide-vue-next'

interface Props {
  title: string
  description?: string
}
defineProps<Props>()
</script>

<template>
  <header class="border-b bg-card">
    <div class="container flex items-center gap-4 py-4">
      <NuxtLink
        to="/"
        class="inline-flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground"
      >
        <ChevronLeft class="size-4" />
        Tools
      </NuxtLink>
      <div class="flex-1">
        <h1 class="text-2xl font-semibold tracking-tight">{{ title }}</h1>
        <p v-if="description" class="text-sm text-muted-foreground mt-0.5">
          {{ description }}
        </p>
      </div>
    </div>
  </header>
</template>
