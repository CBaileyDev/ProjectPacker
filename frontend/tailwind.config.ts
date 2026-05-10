import type { Config } from "tailwindcss";

export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  darkMode: "class",
  theme: {
    extend: {
      // Layered z-index scale — components use these tokens (e.g.
      // `z-modal`, `z-toast`) instead of hardcoded numbers so a single
      // ordering policy lives in one place.
      zIndex: {
        behind: "-1",
        base: "0",
        dropdown: "50",
        sticky: "100",
        modal: "200",
        popover: "300",
        toast: "400",
      },
      // Bridge CSS custom properties to Tailwind utilities so we can write
      // `bg-success`, `text-warning-foreground`, etc.
      colors: {
        surface: "var(--surface)",
        success: "var(--success)",
        "success-foreground": "var(--success-foreground)",
        warning: "var(--warning)",
        "warning-foreground": "var(--warning-foreground)",
        info: "var(--info)",
        "info-foreground": "var(--info-foreground)",
      },
    },
  },
} satisfies Config;
