import { useLocalStorage } from '@vueuse/core'
import type { Preset, ToolId } from '~/types/pixiekit'

const PRESET_KEY = 'pixiekit:presets:v1'

interface PresetStore {
  [tool: string]: Preset<unknown>[]
}

/**
 * Per-tool preset save/load via localStorage.
 * Each tool has its own list of named presets.
 */
export function useToolPreset<O>(tool: ToolId, defaults: { name: string; options: O }[]) {
  const store = useLocalStorage<PresetStore>(PRESET_KEY, {})

  function ensureSeeded() {
    const list = store.value[tool] ?? []
    if (list.length === 0) {
      const seeded: Preset<unknown>[] = defaults.map(d => ({
        name: d.name,
        tool,
        options: d.options as unknown,
        created_at: Date.now(),
      }))
      store.value = { ...store.value, [tool]: seeded }
    }
  }
  ensureSeeded()

  function list(): Preset<O>[] {
    return (store.value[tool] ?? []) as Preset<O>[]
  }

  function save(name: string, options: O): void {
    const trimmed = name.trim()
    if (trimmed.length === 0) return
    const current = (store.value[tool] ?? []) as Preset<O>[]
    const without = current.filter(p => p.name !== trimmed)
    const next: Preset<O> = {
      name: trimmed,
      tool,
      options,
      created_at: Date.now(),
    }
    store.value = {
      ...store.value,
      [tool]: [...without, next] as unknown as Preset<unknown>[],
    }
  }

  function load(name: string): Preset<O> | undefined {
    return list().find(p => p.name === name)
  }

  function remove(name: string): void {
    const current = (store.value[tool] ?? []) as Preset<O>[]
    store.value = {
      ...store.value,
      [tool]: current.filter(p => p.name !== name) as unknown as Preset<unknown>[],
    }
  }

  return { list, save, load, remove }
}
