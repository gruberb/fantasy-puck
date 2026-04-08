/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      screens: { sm: { max: "640px" } },
      fontFamily: {
        sans: [
          "'Space Grotesk'",
          "system-ui",
          "-apple-system",
          "BlinkMacSystemFont",
          "sans-serif",
        ],
      },
      colors: {
        brutal: {
          black: "#1A1A1A",
          white: "#FAFAFA",
          cream: "#F5F0E8",
          blue: "#2563EB",
          red: "#EF4444",
          yellow: "#FACC15",
          pink: "#EC4899",
          teal: "#14B8A6",
          orange: "#F97316",
          purple: "#8B5CF6",
          gray: "#6B7280",
        },
        // Keep NHL colors for team-specific usage
        "nhl-blue": "#041E42",
        "nhl-red": "#AF1E2D",
        "nhl-light-blue": "#6BBBAE",
        "nhl-gold": "#FFB81C",
      },
      boxShadow: {
        brutal: "4px 4px 0px 0px #1A1A1A",
        "brutal-sm": "2px 2px 0px 0px #1A1A1A",
        "brutal-blue": "4px 4px 0px 0px #2563EB",
        "brutal-red": "4px 4px 0px 0px #EF4444",
        "brutal-yellow": "4px 4px 0px 0px #FACC15",
        "brutal-pink": "4px 4px 0px 0px #EC4899",
        "brutal-teal": "4px 4px 0px 0px #14B8A6",
        "brutal-orange": "4px 4px 0px 0px #F97316",
        "brutal-purple": "4px 4px 0px 0px #8B5CF6",
      },
      animation: {
        "pulse-live": "pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite",
      },
      keyframes: {
        pulse: {
          "0%, 100%": { opacity: 1 },
          "50%": { opacity: 0.5 },
        },
      },
    },
  },
  plugins: [],
};
