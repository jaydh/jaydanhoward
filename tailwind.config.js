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
      },
    },
  },
  plugins: [],
};
