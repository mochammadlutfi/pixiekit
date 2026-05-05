<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { CheckCircle2, AlertCircle, Loader2 } from 'lucide-vue-next'

interface LogLine {
  id: number
  text: string
  status: 'ok' | 'error' | 'info'
}

interface Props {
  lines: LogLine[]
  current: number
  total: number
  running: boolean
}

const props = defineProps<Props>()

const percent = computed(() => {
  if (props.total === 0) return 0
  return Math.min(100, Math.round((props.current / props.total) * 100))
})

const scroller = ref<HTMLDivElement>()

watch(
  () => props.lines.length,
  async () => {
    await nextTick()
    if (scroller.value) {
      scroller.value.scrollTop = scroller.value.scrollHeight
    }
  },
)
</script>

<template>
  <div class="rounded-lg border bg-card">
    <header class="flex items-center justify-between border-b px-4 py-3">
      <h2 class="text-sm font-semibold">Progress</h2>
      <span class="text-xs tabular-nums text-muted-foreground">
        {{ current }} / {{ total }} ({{ percent }}%)
      </span>
    </header>
    <div class="px-4 pt-3">
      <div class="h-2 w-full overflow-hidden rounded-full bg-muted">
        <div
          class="h-full rounded-full bg-primary transition-all"
          :style="{ width: `${percent}%` }"
        />
      </div>
    </div>
    <div
      ref="scroller"
      class="h-48 overflow-auto border-t mt-3 p-3 font-mono text-xs space-y-1"
    >
      <p v-if="lines.length === 0" class="text-muted-foreground italic">
        No activity yet — click Process to begin.
      </p>
      <div
        v-for="line in lines"
        :key="line.id"
        class="flex items-center gap-2"
      >
        <Loader2
          v-if="line.status === 'info'"
          class="size-3.5 shrink-0 text-muted-foreground"
        />
        <CheckCircle2
          v-else-if="line.status === 'ok'"
          class="size-3.5 shrink-0 text-emerald-600"
        />
        <AlertCircle
          v-else
          class="size-3.5 shrink-0 text-red-600"
        />
        <span :class="{ 'text-red-600': line.status === 'error' }">{{ line.text }}</span>
      </div>
    </div>
  </div>
</template>
