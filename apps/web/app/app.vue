<script setup lang="ts">
const api = usePixiekitApi()
</script>

<template>
  <div class="min-h-screen bg-background text-foreground">
    <nav class="border-b bg-card">
      <div class="container flex h-14 items-center justify-between gap-4">
        <NuxtLink to="/" class="flex items-center gap-2 font-semibold">
          <span class="inline-block size-6 rounded-md bg-primary" />
          Pixiekit
        </NuxtLink>
        <span
          class="text-xs px-2 py-1 rounded-md font-mono"
          :class="api.mode === 'mock' ? 'bg-amber-100 text-amber-900' : 'bg-emerald-100 text-emerald-900'"
        >
          API: {{ api.mode }}{{ api.mode === 'real' ? ` (${api.baseUrl})` : '' }}
        </span>
      </div>
    </nav>
    <NuxtPage />
  </div>
</template>
