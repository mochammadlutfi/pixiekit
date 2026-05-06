<script setup lang="ts">
import { Sparkles } from 'lucide-vue-next'
const api = usePixiekitApi()
</script>

<template>
  <div class="min-h-screen bg-background text-foreground">
    <nav
      class="glass sticky top-0 z-40 border-b border-border/60"
      aria-label="Primary"
    >
      <div class="container flex h-14 items-center justify-between gap-4">
        <NuxtLink
          to="/"
          class="flex items-center gap-2.5 font-semibold tracking-tight rounded-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring px-1 -mx-1"
        >
          <span class="gradient-brand inline-flex size-7 items-center justify-center rounded-lg shadow-sm">
            <Sparkles class="size-4 text-white" />
          </span>
          <span class="text-[15px]">Pixiekit</span>
        </NuxtLink>

        <div class="flex items-center gap-2">
          <span
            class="inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[11px] font-medium tabular-nums"
            :class="
              api.mode === 'mock'
                ? 'border-warning/30 bg-warning/10 text-warning-foreground/90 dark:text-warning'
                : 'border-success/30 bg-success/10 text-success/90 dark:text-success'
            "
            :title="api.mode === 'real' ? api.baseUrl : 'Mock data — set NUXT_PUBLIC_PIXIEKIT_API_URL'"
          >
            <span
              class="size-1.5 rounded-full"
              :class="api.mode === 'mock' ? 'bg-warning animate-pulse-soft' : 'bg-success animate-pulse-soft'"
            />
            {{ api.mode === 'mock' ? 'Mock' : 'Live' }}
            <span v-if="api.mode === 'real'" class="hidden sm:inline text-muted-foreground/80 font-mono ml-1">
              {{ api.baseUrl }}
            </span>
          </span>
        </div>
      </div>
    </nav>
    <NuxtPage />
  </div>
</template>
