<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { CheckCircle2, AlertCircle, Activity, Loader2 } from 'lucide-vue-next'

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
  <div class="surface overflow-hidden rounded-xl border border-border/70">
    <header class="flex items-center justify-between gap-3 border-b border-border/60 px-4 py-3">
      <div class="flex items-center gap-2">
        <span
          class="inline-flex size-7 items-center justify-center rounded-md transition-colors"
          :class="running ? 'bg-primary/10 text-primary' : 'bg-muted text-muted-foreground'"
        >
          <Loader2 v-if="running" class="size-4 animate-spin" />
          <Activity v-else class="size-3.5" />
        </span>
        <h2 class="text-sm font-semibold tracking-tight">Progress</h2>
      </div>
      <span class="text-xs tabular-nums text-muted-foreground font-mono">
        {{ current }} / {{ total }}
        <span class="ml-1 text-foreground font-semibold">{{ percent }}%</span>
      </span>
    </header>

    <div class="px-4 pt-3">
      <div
        class="relative h-1.5 w-full overflow-hidden rounded-full bg-muted"
        role="progressbar"
        :aria-valuenow="percent"
        aria-valuemin="0"
        aria-valuemax="100"
      >
        <div
          class="h-full rounded-full bg-gradient-to-r from-primary via-primary to-fuchsia-500 transition-all duration-300 ease-out"
          :style="{ width: `${percent}%` }"
        />
        <div
          v-if="running"
          class="absolute inset-y-0 left-0 h-full rounded-full bg-[linear-gradient(90deg,transparent,rgba(255,255,255,0.45),transparent)] bg-[length:200%_100%] animate-shimmer"
          :style="{ width: `${percent}%` }"
        />
      </div>
    </div>

    <div
      ref="scroller"
      class="scroll-thin mt-3 max-h-56 overflow-auto border-t border-border/60 px-4 py-3 font-mono text-[11px] leading-relaxed"
    >
      <p v-if="lines.length === 0" class="text-muted-foreground italic">
        Belum ada aktivitas — klik <span class="not-italic font-semibold">Process</span> untuk mulai.
      </p>
      <ul class="space-y-1">
        <li
          v-for="line in lines"
          :key="line.id"
          class="flex items-start gap-2 animate-fade-in"
        >
          <Loader2
            v-if="line.status === 'info'"
            class="mt-0.5 size-3.5 shrink-0 text-muted-foreground"
          />
          <CheckCircle2
            v-else-if="line.status === 'ok'"
            class="mt-0.5 size-3.5 shrink-0 text-success"
          />
          <AlertCircle
            v-else
            class="mt-0.5 size-3.5 shrink-0 text-destructive"
          />
          <span
            class="break-all"
            :class="{
              'text-destructive': line.status === 'error',
              'text-foreground/90': line.status !== 'error',
            }"
          >
            {{ line.text }}
          </span>
        </li>
      </ul>
    </div>
  </div>
</template>
