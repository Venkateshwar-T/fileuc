/** @type {import('tailwindcss').Config} */
export default {
  darkMode: 'class',
  content: ['./index.html', './src/**/*.{svelte,js,ts}'],
  theme: {
    extend: {
      fontFamily: {
        sans: [
          'Segoe UI Variable Text',
          'Segoe UI',
          '-apple-system',
          'BlinkMacSystemFont',
          'sans-serif',
        ],
        display: [
          'Segoe UI Variable Display',
          'Segoe UI',
          '-apple-system',
          'BlinkMacSystemFont',
          'sans-serif',
        ],
      },
      colors: {
        accent: {
          DEFAULT: '#7066E0',
          hover: '#8076EE',
        },
        danger: {
          DEFAULT: '#EF4444',
          hover: '#DC2626',
          bg: '#521E24',
          text: '#FF8080',
        },
        success: {
          DEFAULT: '#22C55E',
        },
        dark: {
          bg: 'var(--color-bg)',
          sidebar: 'var(--color-sidebar)',
          card: 'var(--color-card)',
          border: 'var(--color-border)',
          control: 'var(--color-control)',
        },
      },
    },
  },
  plugins: [],
};
