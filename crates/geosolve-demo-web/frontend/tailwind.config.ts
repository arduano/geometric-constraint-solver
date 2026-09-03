// SPDX-License-Identifier: GPL-3.0-or-later
import type { Config } from "tailwindcss";

export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        canvas: "hsl(var(--canvas))",
        surface: "hsl(var(--surface))",
        raised: "hsl(var(--raised))",
        border: "hsl(var(--border))",
        foreground: "hsl(var(--foreground))",
        muted: "hsl(var(--muted))",
        accent: "hsl(var(--accent))",
        danger: "hsl(var(--danger))",
      },
      boxShadow: {
        panel: "0 12px 36px rgb(0 0 0 / .38)",
      },
    },
  },
  plugins: [],
} satisfies Config;
