/** @type {import('tailwindcss').Config} */
export default {
  content: {
    files: ["./src/**/*.rs"],
  },
  plugins: [require("daisyui")],
  daisyui: {
    themes: [{
      light: {
        ...require("daisyui/src/theming/themes")["[data-theme=corporate]"],
        "--rounded-box": "1rem",
        "--rounded-btn": ".5rem",
        "--rounded-badge": "1.9rem",
      },
    }, "night"],
  },
  theme: {
    extend: {
      gridTemplateColumns: {
        'entry': '2.5rem 1fr 5rem 10rem 7rem',
        'entry-mobile': '2.5rem 1fr 5rem',
      }
    }
  }
}
