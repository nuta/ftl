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
    ],

    socialLinks: [
      { icon: 'github', link: 'https://github.com/nuta/ftl' }
    ]
  }
})
