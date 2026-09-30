import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'blewred',
  description: 'Preemptive Live Stream Protection Suite for Content Creators',
  base: '/blewred/',
  lastUpdated: true,
  cleanUrls: true,

  locales: {
    root: {
      label: 'English',
      lang: 'en',
      themeConfig: {
        nav: [
          { text: 'Architecture & Overview', link: '/' },
          { text: 'Streamer Quick Start', link: '/quickstart_streamer' },
          { text: 'GitHub', link: 'https://github.com/msbluesnow/blewred' }
        ],
        sidebar: [
          {
            text: 'Documentation',
            items: [
              { text: 'Architecture & Overview', link: '/' },
              { text: 'Streamer Quick Start', link: '/quickstart_streamer' }
            ]
          }
        ],
        footer: {
          message: 'Released under the Apache 2.0 License.',
          copyright: 'Copyright © 2026 msbluesnow & blewred Contributors'
        }
      }
    },
    ru: { 
      label: 'Русский',
      lang: 'ru',
      link: '/ru/',
      themeConfig: {
        nav: [
          { text: 'Архитектура и обзор', link: '/ru/' },
          { text: 'Гайд для стримеров', link: '/ru/quickstart_streamer' },
          { text: 'GitHub', link: 'https://github.com/msbluesnow/blewred' }
        ],
        sidebar: [
          {
            text: 'Документация',
            items: [
              { text: 'Архитектура и обзор', link: '/ru/' },
              { text: 'Гайд для стримеров', link: '/ru/quickstart_streamer' }
            ]
          }
        ],
        footer: {
          message: 'Распространяется по лицензии Apache 2.0.',
          copyright: 'Copyright © 2024-2026 msbluesnow & авторы blewred'
        }
      }
    }
  },

  themeConfig: {
    socialLinks: [
      { icon: 'github', link: 'https://github.com/msbluesnow/blewred' }
    ],
    search: {
      provider: 'local'
    }
  }
})
