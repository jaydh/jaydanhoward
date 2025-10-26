/** @type {import('tailwindcss').Config} */
module.exports = {
  darkMode: "class",
  content: {
    files: ["*.html", "./src/**/*.rs"],
  },
  theme: {
    extend: {
      colors: {
        "charcoal": "#1E1E1E",
        "gray": "#E1E1E1",
        "accent": "#3B82F6",
        "accent-dark": "#2563EB",
        "accent-light": "#60A5FA",
        "surface": "#FFFFFF",
        "surface-dark": "#111111",
        "border": "#E5E7EB",
        "border-dark": "#374151",
      },
      boxShadow: {
        'minimal': '0 1px 3px 0 rgba(0, 0, 0, 0.1), 0 1px 2px 0 rgba(0, 0, 0, 0.06)',
        'minimal-lg': '0 10px 15px -3px rgba(0, 0, 0, 0.05), 0 4px 6px -2px rgba(0, 0, 0, 0.03)',
        'minimal-xl': '0 20px 25px -5px rgba(0, 0, 0, 0.05), 0 10px 10px -5px rgba(0, 0, 0, 0.02)',
      }
    },
  },
  plugins: [],
};
