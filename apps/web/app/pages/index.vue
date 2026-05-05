<script setup lang="ts">
import { Eraser, Wand2, Film, ArrowRight } from 'lucide-vue-next'
import type { ToolMeta } from '~/types/pixiekit'

const tools: (ToolMeta & { icon: unknown; tint: string })[] = [
  {
    id: 'bg-remove',
    title: 'BG Remove',
    description: 'Chroma key + despill + alpha erode for AI-generated character renders.',
    href: '/bg-remove',
    icon: Eraser,
    tint: 'bg-emerald-100 text-emerald-700',
  },
  {
    id: 'vectorize',
    title: 'Vectorize',
    description: 'Trace PNG/JPG to SVG via vtracer. Smoothness slider for quick wins.',
    href: '/vectorize',
    icon: Wand2,
    tint: 'bg-violet-100 text-violet-700',
  },
  {
    id: 'video-to-sprite',
    title: 'Video → Sprite',
    description: 'Extract frames from MP4/MOV/WebM and stitch a horizontal sprite sheet.',
    href: '/video-to-sprite',
    icon: Film,
    tint: 'bg-amber-100 text-amber-700',
  },
]

useHead({ title: 'Pixiekit — Asset Toolkit' })
</script>

<template>
  <main class="container py-10">
    <header class="mb-8 max-w-2xl">
      <h1 class="text-3xl font-bold tracking-tight">Asset toolkit</h1>
      <p class="mt-2 text-muted-foreground">
        Three tools for cleaning, vectorising, and animating game-ready assets. Pick a tool to begin.
      </p>
    </header>

    <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
      <NuxtLink
        v-for="tool in tools"
        :key="tool.id"
        :to="tool.href"
        class="group rounded-lg border bg-card p-5 transition-colors hover:border-primary/40 hover:bg-accent/40"
      >
        <div
          class="mb-4 inline-flex size-10 items-center justify-center rounded-md"
          :class="tool.tint"
        >
          <component :is="tool.icon" class="size-5" />
        </div>
        <h2 class="text-lg font-semibold">{{ tool.title }}</h2>
        <p class="mt-1 text-sm text-muted-foreground">{{ tool.description }}</p>
        <span class="mt-4 inline-flex items-center gap-1 text-sm font-medium text-primary">
          Open
          <ArrowRight class="size-4 transition-transform group-hover:translate-x-0.5" />
        </span>
      </NuxtLink>
    </div>
  </main>
</template>
