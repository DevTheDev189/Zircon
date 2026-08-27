/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{vue,js}'],
  theme: {
    extend: {
      colors: {
        void: '#070b0f',
        bg: '#070b0f',
        surface: '#0d1117',
        card: '#101820',
        edge: '#263545',
        accent: '#47d2c9',
        'accent-bright': '#5adfd5',
        'accent-deep': '#20b2aa',
        'accent-ink': '#022623',
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
