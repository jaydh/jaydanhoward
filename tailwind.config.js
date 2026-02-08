/** @type {import('tailwindcss').Config} */
module.exports = {
  darkMode: 'class',
  content: {
    files: ["*.html", "./src/**/*.rs"],
  },
  theme: {
    extend: {
      colors: {
        "charcoal": "rgb(var(--color-charcoal) / <alpha-value>)",
        "charcoal-light": "rgb(var(--color-charcoal-light) / <alpha-value>)",
        "charcoal-lighter": "rgb(var(--color-charcoal-lighter) / <alpha-value>)",
        "gray": "rgb(var(--color-gray) / <alpha-value>)",
        "accent": "rgb(var(--color-accent) / <alpha-value>)",
        "accent-dark": "rgb(var(--color-accent-dark) / <alpha-value>)",
        "surface": "rgb(var(--color-surface) / <alpha-value>)",
        "border": "rgb(var(--color-border) / <alpha-value>)",
      },
      boxShadow: {
        'minimal': '0 1px 3px 0 rgba(0, 0, 0, 0.1), 0 1px 2px 0 rgba(0, 0, 0, 0.06)',
        'minimal-lg': '0 10px 15px -3px rgba(0, 0, 0, 0.05), 0 4px 6px -2px rgba(0, 0, 0, 0.03)',
        'minimal-xl': '0 20px 25px -5px rgba(0, 0, 0, 0.05), 0 10px 10px -5px rgba(0, 0, 0, 0.02)',
      },
      keyframes: {
        shimmer: {
          '0%': { transform: 'translateX(-100%)' },
          '100%': { transform: 'translateX(100%)' },
        },
      },
      animation: {
        shimmer: 'shimmer 2s infinite',
      },
    },
  },
  plugins: [],
};
