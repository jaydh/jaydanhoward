/** @type {import('tailwindcss').Config} */
module.exports = {
  darkMode: "class",
  content: {
    files: ["*.html", "./src/**/*.rs"],
  },
  theme: {
    extend: {
      colors: {
        "ivory-beige": "#F5EEDD",
        "pale-beige": "#E6D7B9",
        "warm-beige": "#D2B48C",
      },
    },
  },
  plugins: [],
};
