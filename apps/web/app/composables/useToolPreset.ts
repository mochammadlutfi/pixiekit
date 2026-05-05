import { shallowRef, triggerRef } from 'vue'
import type { Preset, ToolId } from '~/types/pixiekit'

/**
 * Per-tool preset save/load. Uses the active `usePixiekitApi()` client, so
 * mock mode hits localStorage and real mode hits the `/api/presets` endpoints.
 *
 * The composable exposes a synchronous `list()` / `load()` reading a reactive
 * cache, plus async `save()` / `remove()` that mutate the backing store and
 * refresh the cache. This preserves the existing component call-sites.
 *
 * Defaults seed only in mock mode (and only the first time the cache for this
 * tool is empty). In real mode the server is the source of truth — empty list
 * means "no presets yet", not "show defaults".
 */
export function useToolPreset<O>(
  tool: ToolId,
  defaults: { name: string; options: O }[],
) {
  const api = usePixiekitApi()
  // shallowRef avoids deep-unwrapping the generic `O` so the Preset<O> shape
  // round-trips cleanly through TypeScript's ref unwrapping.
  const cache = shallowRef<Preset<O>[]>([])

  async function refresh() {
    const all = await api.client.listPresets()
    const forTool = all.filter(p => p.tool === tool) as Preset<O>[]
    if (api.mode === 'mock' && forTool.length === 0 && defaults.length > 0) {
      // Seed defaults locally for first-time mock-mode dev. In real mode we
      // never auto-write to the server.
      for (const d of defaults) {
        await api.client.savePreset(d.name, tool, d.options)
      }
      const reseed = await api.client.listPresets()
      cache.value = reseed.filter(p => p.tool === tool) as Preset<O>[]
      triggerRef(cache)
      return
    }
    cache.value = forTool
    triggerRef(cache)
  }

  // Fire-and-forget initial load. Components can render with an empty list
  // until refresh resolves, then the reactive cache populates.
  refresh().catch(err => {
    console.warn(`[useToolPreset:${tool}] initial refresh failed:`, err)
  })

  function list(): Preset<O>[] {
    return cache.value
  }

  function load(name: string): Preset<O> | undefined {
    return cache.value.find(p => p.name === name)
  }

  async function save(name: string, options: O): Promise<void> {
    const trimmed = name.trim()
    if (trimmed.length === 0) return
    await api.client.savePreset(trimmed, tool, options)
    await refresh()
  }

  async function remove(name: string): Promise<void> {
    await api.client.deletePreset(name)
    await refresh()
  }

  return { list, save, load, remove, refresh }
}
