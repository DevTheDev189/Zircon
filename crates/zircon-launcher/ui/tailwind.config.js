/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{vue,js}'],
  theme: {
    extend: {
      colors: {
        void: 'rgb(var(--color-bg-rgb, 7 11 15) / <alpha-value>)',
        bg: 'rgb(var(--color-bg-rgb, 7 11 15) / <alpha-value>)',
        sidebar: 'rgb(var(--color-sidebar-rgb, 10 15 20) / <alpha-value>)',
        surface: 'rgb(var(--color-card-rgb, 14 22 34) / <alpha-value>)',
        card: 'rgb(var(--color-card-rgb, 14 22 34) / <alpha-value>)',
        well: 'rgb(var(--color-well-rgb, 7 11 16) / <alpha-value>)',
        edge: 'rgb(var(--color-border-rgb, 38 53 69) / <alpha-value>)',
        accent: 'rgb(var(--color-accent-rgb, 71 210 201) / <alpha-value>)',
        'accent-bright': 'rgb(var(--color-accent-bright-rgb, 90 223 213) / <alpha-value>)',
        'accent-deep': 'rgb(var(--color-accent-deep-rgb, 32 178 170) / <alpha-value>)',
        'accent-ink': 'rgb(var(--color-accent-ink-rgb, 2 38 35) / <alpha-value>)',
        cyan: {
          300: 'rgb(var(--color-accent-bright-rgb, 90 223 213) / <alpha-value>)',
          400: 'rgb(var(--color-accent-rgb, 71 210 201) / <alpha-value>)',
          500: 'rgb(var(--color-accent-deep-rgb, 32 178 170) / <alpha-value>)',
        },
        muted: '#8b949e',
        text: '#c9d1d9',
        danger: '#ef4444',
      },
      fontFamily: {
        sans: ['"Segoe UI"', 'system-ui', '-apple-system', 'sans-serif'],
      },
    },
  },
  plugins: [],
};
