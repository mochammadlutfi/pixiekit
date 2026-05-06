<script setup lang="ts">
import { ChevronLeft } from 'lucide-vue-next'

interface Props {
  title: string
  description?: string
  /** Optional eyebrow text (e.g. "Step 1 of 3" or category) */
  eyebrow?: string
}
defineProps<Props>()
</script>

<template>
  <header class="border-b bg-card/50">
    <div class="container py-6 lg:py-8">
      <NuxtLink
        to="/"
        class="group inline-flex items-center gap-1 text-xs font-medium text-muted-foreground hover:text-foreground transition-colors"
      >
        <ChevronLeft class="size-3.5 transition-transform group-hover:-translate-x-0.5" />
        Back to tools
      </NuxtLink>
      <div class="mt-3 flex flex-col gap-1">
        <p
          v-if="eyebrow"
          class="text-[11px] font-semibold uppercase tracking-[0.14em] text-primary/80"
        >
          {{ eyebrow }}
        </p>
        <h1 class="text-2xl font-bold tracking-tight sm:text-3xl">
          {{ title }}
        </h1>
        <p
          v-if="description"
          class="text-sm text-muted-foreground max-w-2xl"
        >
          {{ description }}
        </p>
      </div>
      <slot />
    </div>
  </header>
</template>
