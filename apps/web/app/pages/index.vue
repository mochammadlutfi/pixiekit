<script setup lang="ts">
import {
  ArrowRight,
  AudioWaveform,
  Crop,
  Eraser,
  FileArchive,
  FileCode2,
  Film,
  Layout,
  Maximize2,
  Package,
  Play,
  Sparkles,
  Wand2,
  Zap,
} from 'lucide-vue-next'
import type { ToolMeta } from '~/types/pixiekit'

const tools: (ToolMeta & { icon: unknown; tint: string; ring: string })[] = [
  {
    id: 'bg-remove',
    title: 'BG Remove',
    description: 'Chroma key + despill + alpha erode untuk render karakter AI.',
    href: '/bg-remove',
    icon: Eraser,
    tint: 'bg-emerald-50 text-emerald-700 dark:bg-emerald-500/10 dark:text-emerald-300',
    ring: 'group-hover:ring-emerald-500/30',
  },
  {
    id: 'vectorize',
    title: 'Vectorize',
    description: 'Trace PNG/JPG ke SVG via vtracer. Slider smoothness untuk hasil cepat.',
    href: '/vectorize',
    icon: Wand2,
    tint: 'bg-violet-50 text-violet-700 dark:bg-violet-500/10 dark:text-violet-300',
    ring: 'group-hover:ring-violet-500/30',
  },
  {
    id: 'video-to-sprite',
    title: 'Video → Sprite',
    description: 'Ekstrak frame dari MP4/MOV/WebM dan jahit jadi sprite sheet horizontal.',
    href: '/video-to-sprite',
    icon: Film,
    tint: 'bg-amber-50 text-amber-700 dark:bg-amber-500/10 dark:text-amber-300',
    ring: 'group-hover:ring-amber-500/30',
  },
  {
    id: 'atlas-pack',
    title: 'Atlas Pack',
    description: 'Pack folder PNG ke 1 texture atlas + Flame TexturePacker JSON.',
    href: '/atlas-pack',
    icon: Package,
    tint: 'bg-sky-50 text-sky-700 dark:bg-sky-500/10 dark:text-sky-300',
    ring: 'group-hover:ring-sky-500/30',
  },
  {
    id: 'optimize',
    title: 'Optimize',
    description: 'Kecilkan PNG/JPG/WebP via oxipng + format convert + strip metadata.',
    href: '/optimize',
    icon: FileArchive,
    tint: 'bg-rose-50 text-rose-700 dark:bg-rose-500/10 dark:text-rose-300',
    ring: 'group-hover:ring-rose-500/30',
  },
  {
    id: 'scale',
    title: 'Scale',
    description: 'Generate multi-resolusi @1x/@2x/@3x (Flutter / iOS suffix / nested).',
    href: '/scale',
    icon: Maximize2,
    tint: 'bg-teal-50 text-teal-700 dark:bg-teal-500/10 dark:text-teal-300',
    ring: 'group-hover:ring-teal-500/30',
  },
  {
    id: 'audio',
    title: 'Audio',
    description: 'LUFS normalize, trim silence, format convert via ffmpeg (OGG/OPUS/MP3/WAV).',
    href: '/audio',
    icon: AudioWaveform,
    tint: 'bg-indigo-50 text-indigo-700 dark:bg-indigo-500/10 dark:text-indigo-300',
    ring: 'group-hover:ring-indigo-500/30',
  },
  {
    id: 'trim-pad',
    title: 'Trim & Pad',
    description: 'Auto-crop transparan border + uniform padding. Pre-step untuk atlas.',
    href: '/trim-pad',
    icon: Crop,
    tint: 'bg-orange-50 text-orange-700 dark:bg-orange-500/10 dark:text-orange-300',
    ring: 'group-hover:ring-orange-500/30',
  },
  {
    id: 'svg-optimize',
    title: 'SVG Optimize',
    description: 'Minify SVG: round koordinat, strip metadata, drop hidden elements.',
    href: '/svg-optimize',
    icon: FileCode2,
    tint: 'bg-lime-50 text-lime-700 dark:bg-lime-500/10 dark:text-lime-300',
    ring: 'group-hover:ring-lime-500/30',
  },
  {
    id: 'nine-slice',
    title: 'Nine Slice',
    description: 'Split image jadi 9 bagian atau generate metadata JSON untuk scaling.',
    href: '/nine-slice',
    icon: Layout,
    tint: 'bg-cyan-50 text-cyan-700 dark:bg-cyan-500/10 dark:text-cyan-300',
    ring: 'group-hover:ring-cyan-500/30',
  },
  {
    id: 'anim-preview',
    title: 'Anim Preview',
    description: 'Konversi sprite sheet atau frame folder jadi animasi GIF/MP4/WebM.',
    href: '/anim-preview',
    icon: Play,
    tint: 'bg-fuchsia-50 text-fuchsia-700 dark:bg-fuchsia-500/10 dark:text-fuchsia-300',
    ring: 'group-hover:ring-fuchsia-500/30',
  },
]

useHead({ title: 'Pixiekit — Asset Toolkit' })
</script>

<template>
  <main class="container py-10 lg:py-16">
    <section class="mb-10 max-w-3xl lg:mb-14">
      <span
        class="inline-flex items-center gap-1.5 rounded-full border border-primary/20 bg-primary/5 px-2.5 py-1 text-[11px] font-semibold uppercase tracking-[0.12em] text-primary"
      >
        <Sparkles class="size-3" />
        Asset toolkit
      </span>
      <h1 class="mt-4 text-4xl font-bold tracking-tight sm:text-5xl">
        Bersihkan, vektorisasi, dan
        <span class="gradient-text-brand">animasikan asset</span>
        siap pakai.
      </h1>
      <p class="mt-4 max-w-2xl text-base text-muted-foreground sm:text-lg">
        Tiga tool fokus untuk pipeline game dan animasi. Drop file, atur opsi, download hasilnya — tanpa setup output folder.
      </p>
      <div class="mt-6 flex flex-wrap items-center gap-3 text-xs text-muted-foreground">
        <span class="inline-flex items-center gap-1.5">
          <Zap class="size-3.5 text-primary" /> Local-first
        </span>
        <span aria-hidden="true">•</span>
        <span>No upload to cloud</span>
        <span aria-hidden="true">•</span>
        <span>Pure Rust core</span>
      </div>
    </section>

    <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
      <NuxtLink
        v-for="tool in tools"
        :key="tool.id"
        :to="tool.href"
        class="group relative flex flex-col rounded-xl border bg-card p-6 ring-1 ring-transparent transition-all duration-200 ease-spring hover:-translate-y-0.5 hover:border-border hover:shadow-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        :class="tool.ring"
      >
        <div
          class="mb-4 inline-flex size-11 items-center justify-center rounded-lg transition-transform duration-200 group-hover:scale-110"
          :class="tool.tint"
        >
          <component :is="tool.icon" class="size-5" />
        </div>
        <h2 class="text-base font-semibold tracking-tight">{{ tool.title }}</h2>
        <p class="mt-1 flex-1 text-sm text-muted-foreground">{{ tool.description }}</p>
        <span
          class="mt-5 inline-flex items-center gap-1 text-sm font-medium text-primary"
        >
          Open
          <ArrowRight class="size-4 transition-transform duration-200 group-hover:translate-x-1" />
        </span>
      </NuxtLink>
    </div>
  </main>
</template>
