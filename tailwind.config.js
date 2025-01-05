import daisyui from 'daisyui'
import tailwindcss from 'tailwindcss'

import themes from 'daisyui/src/theming/themes'

/** @type {tailwindcss.Config} */
export default {
  content: {
    files: ["./app/**/*.rs"],
  },
  plugins: [daisyui],
  daisyui: {
    themes: [
      {
        light: {
          ...themes["corporate"],
          "--rounded-box": "1rem",
          "--rounded-btn": ".5rem",
          "--rounded-badge": "1.9rem",
        },
      },
      "night"
    ],
    darkTheme: "night",
  },
  theme: {
    extend: {
      gridTemplateColumns: {
        'entry': '2.5rem 1fr 6rem 8rem',
        'entry-mobile': '2.5rem 1fr 6rem',
      }
    }
  }
}
