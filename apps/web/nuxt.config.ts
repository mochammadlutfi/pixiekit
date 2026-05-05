// https://nuxt.com/docs/api/configuration/nuxt-config
export default defineNuxtConfig({
  compatibilityDate: '2025-04-01',
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
      ],
    },
  },
})
