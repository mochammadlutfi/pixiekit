<script setup lang="ts">
interface Props {
  title?: string
  subtitle?: string
  /** Render without internal padding for full-bleed content (e.g. file lists) */
  flush?: boolean
}

withDefaults(defineProps<Props>(), {
  flush: false,
})
</script>

<template>
  <section
    class="surface group overflow-hidden rounded-xl border border-border/70 transition-shadow hover:shadow-sm"
  >
    <header
      v-if="title || $slots.actions"
      class="flex items-start justify-between gap-3 border-b border-border/60 px-4 py-3 sm:px-5"
    >
      <div class="min-w-0">
        <h2
          v-if="title"
          class="text-sm font-semibold tracking-tight"
        >
          {{ title }}
        </h2>
        <p
          v-if="subtitle"
          class="mt-0.5 text-xs text-muted-foreground"
        >
          {{ subtitle }}
        </p>
      </div>
      <div v-if="$slots.actions" class="flex shrink-0 items-center gap-2">
        <slot name="actions" />
      </div>
    </header>
    <div :class="flush ? '' : 'space-y-4 p-4 sm:p-5'">
      <slot />
    </div>
  </section>
</template>
