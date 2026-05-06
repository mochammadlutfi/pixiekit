import type { ApiClient } from '~/lib/api-client'
import { createApiClient } from '~/lib/api-client'
import { createMockApiClient } from '~/lib/mock-api'
import { createTauriApiClient } from '~/lib/tauri-client'

interface UsePixiekitApi {
  client: ApiClient
  mode: 'mock' | 'real' | 'tauri'
  baseUrl: string
}

let cached: UsePixiekitApi | null = null

export function usePixiekitApi(): UsePixiekitApi {
  if (cached) return cached

  // Check if running in Tauri
  const isTauri = typeof window !== 'undefined' && (window as any).__TAURI_INTERNALS__ !== undefined

  if (isTauri) {
    cached = { client: createTauriApiClient(), mode: 'tauri', baseUrl: 'tauri://localhost' }
    return cached
  }

  const config = useRuntimeConfig()
  const baseUrl = String(config.public.pixiekitApiUrl || '').trim()
  if (baseUrl.length > 0) {
    cached = { client: createApiClient(baseUrl), mode: 'real', baseUrl }
  } else {
    cached = { client: createMockApiClient(), mode: 'mock', baseUrl: '' }
  }
  return cached
}
