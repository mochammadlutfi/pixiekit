<script setup lang="ts">
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
  <div class="grid grid-cols-2 gap-2">
    <div class="space-y-1.5">
      <p class="text-xs font-medium uppercase tracking-wide text-muted-foreground">Before</p>
      <div
        class="aspect-square w-full overflow-hidden rounded-md border bg-muted/30 flex items-center justify-center"
      >
        <img
          v-if="before"
          :src="before"
          alt="Before"
          class="max-h-full max-w-full object-contain"
        />
        <span v-else class="text-xs text-muted-foreground">No input</span>
      </div>
    </div>
    <div class="space-y-1.5">
      <p class="text-xs font-medium uppercase tracking-wide text-muted-foreground">After</p>
      <div
        class="aspect-square w-full overflow-hidden rounded-md border flex items-center justify-center"
        :class="showCheckerOnAfter ? 'bg-checker' : 'bg-muted/30'"
      >
        <img
          v-if="after"
          :src="after"
          alt="After"
          class="max-h-full max-w-full object-contain"
        />
        <span v-else class="text-xs text-muted-foreground">No output yet</span>
      </div>
    </div>
  </div>
</template>
