/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        fleet: {
          bg: "#0d1117",
          panel: "#161b22",
          border: "#30363d",
          text: "#e6edf3",
          muted: "#8b949e",
          green: "#3fb950",
          red: "#f85149",
          yellow: "#d29922",
        },
      },
    },
  },
  plugins: [],
};
