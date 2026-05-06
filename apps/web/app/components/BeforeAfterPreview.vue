<script setup lang="ts">
import { ImageOff } from 'lucide-vue-next'

interface Props {
  before?: string
  after?: string
  showCheckerOnAfter?: boolean
}

withDefaults(defineProps<Props>(), {
  showCheckerOnAfter: true,
})
</script>

<template>
  <div class="grid grid-cols-2 gap-3">
    <figure class="space-y-1.5">
      <figcaption class="flex items-center justify-between text-[10px] font-semibold uppercase tracking-[0.12em] text-muted-foreground">
        <span>Before</span>
        <span class="rounded-full bg-muted px-1.5 py-0.5 text-[9px] tracking-wider text-muted-foreground/80">SRC</span>
      </figcaption>
      <div
        class="relative aspect-square w-full overflow-hidden rounded-lg border bg-muted/30"
      >
        <img
          v-if="before"
          :src="before"
          alt="Before"
          class="absolute inset-0 size-full object-contain p-1"
          loading="lazy"
        />
        <div
          v-else
          class="absolute inset-0 flex flex-col items-center justify-center gap-1 text-muted-foreground/70"
        >
          <ImageOff class="size-5" aria-hidden="true" />
          <span class="text-[11px]">Belum ada input</span>
        </div>
      </div>
    </figure>
    <figure class="space-y-1.5">
      <figcaption class="flex items-center justify-between text-[10px] font-semibold uppercase tracking-[0.12em] text-muted-foreground">
        <span>After</span>
        <span class="rounded-full bg-primary/10 px-1.5 py-0.5 text-[9px] tracking-wider text-primary/80">OUT</span>
      </figcaption>
      <div
        class="relative aspect-square w-full overflow-hidden rounded-lg border"
        :class="showCheckerOnAfter ? 'bg-checker' : 'bg-muted/30'"
      >
        <img
          v-if="after"
          :src="after"
          alt="After"
          class="absolute inset-0 size-full object-contain p-1 animate-fade-in"
          loading="lazy"
        />
        <div
          v-else
          class="absolute inset-0 flex flex-col items-center justify-center gap-1 text-muted-foreground/70"
        >
          <ImageOff class="size-5" aria-hidden="true" />
          <span class="text-[11px]">Belum di-process</span>
        </div>
      </div>
    </figure>
  </div>
</template>
