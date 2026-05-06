import { ref, onMounted } from 'vue'

export function useTauri() {
  const isTauri = ref(false)
  const appVersion = ref('')

  onMounted(async () => {
    isTauri.value = typeof window !== 'undefined' && (window as any).__TAURI_INTERNALS__ !== undefined
    
    if (isTauri.value) {
      try {
        const { getVersion } = await import('@tauri-apps/api/app')
        appVersion.value = await getVersion()
      } catch (e) {
        console.warn('Failed to get app version:', e)
      }
    }
  })

  return {
    isTauri,
    appVersion,
  }
}
