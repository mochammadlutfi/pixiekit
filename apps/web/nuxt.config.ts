// https://nuxt.com/docs/api/configuration/nuxt-config
export default defineNuxtConfig({
  compatibilityDate: '2025-04-01',
  ssr: false,
  devtools: { enabled: true },

  modules: [
    '@nuxtjs/tailwindcss',
    '@vueuse/nuxt',
  ],

  css: ['~/assets/css/tailwind.css'],

  typescript: {
    strict: true,
    typeCheck: false, // run separately via `pnpm typecheck`
  },

  runtimeConfig: {
    public: {
      pixiekitApiUrl: process.env.VITE_PIXIEKIT_API_URL || '',
    },
  },

  app: {
    head: {
      title: 'Pixiekit — Asset Toolkit',
      meta: [
        { charset: 'utf-8' },
        { name: 'viewport', content: 'width=device-width, initial-scale=1' },
        { name: 'description', content: 'Local asset preparation toolkit for game and animation pipelines.' },
        { name: 'theme-color', content: '#5B5BF7' },
      ],
      link: [
        { rel: 'preconnect', href: 'https://fonts.googleapis.com' },
        { rel: 'preconnect', href: 'https://fonts.gstatic.com', crossorigin: '' },
        {
          rel: 'stylesheet',
          href: 'https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500&display=swap',
        },
      ],
    },
  },
  vite: {
    clearScreen: false,
    envPrefix: ['VITE_', 'TAURI_'],
    server: {
      strictPort: true,
    },
    build: {
      // Use modern JS target to support destructuring and other features
      target: 'esnext',
      // produce sourcemaps for debug builds
      sourcemap: !!process.env.TAURI_DEBUG,
      rollupOptions: {
        external: [/^\@tauri-apps/],
      },
    },
    optimizeDeps: {
      exclude: ['@tauri-apps/api', '@tauri-apps/plugin-dialog']
    }
  },
})


