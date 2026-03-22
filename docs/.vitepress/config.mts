import { defineConfig } from 'vitepress'

// https://vitepress.dev/reference/site-config
export default defineConfig({
  title: "FTL Documentation",
  description: "Learn FTL",
  cleanUrls: true,
  themeConfig: {
    nav: [
      { text: 'Home', link: '/' },
      { text: 'Getting Started', link: '/getting-started' },
    ],

    sidebar: [
      { text: 'Getting Started', link: '/getting-started' },
      { text: 'Comparisons', link: '/comparisons' },
      { text: 'API', link: '/api/rust' },
      {
        text: 'Learn',
        collapsed: false,
        items: [
          { text: 'Process', link: '/learn/process' },
          { text: 'Isolation', link: '/learn/isolation' },
          { text: 'Channel', link: '/learn/channel' },
          { text: 'Linux Compatibility', link: '/learn/linux-compatibility' },
        ],
      },
    ],

    socialLinks: [
      { icon: 'github', link: 'https://github.com/nuta/ftl' }
    ]
  }
})
