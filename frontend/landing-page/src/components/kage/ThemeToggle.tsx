import { useEffect, useState } from "react";
import { Moon, Sun } from "lucide-react";

type Theme = "light" | "dark";

const STORAGE_KEY = "syncit-theme";

function applyTheme(theme: Theme) {
  const root = document.documentElement;
  root.classList.toggle("dark", theme === "dark");
  root.style.colorScheme = theme;
}

/**
 * Floating light/dark switch below the nav. The choice is stored and wins
 * over the system; with nothing stored, the page follows
 * `prefers-color-scheme` (the class is applied pre-paint by RootShell's
 * inline script) and keeps following it if the system changes.
 */
export function ThemeToggle() {
  const [theme, setTheme] = useState<Theme | null>(null);

  useEffect(() => {
    setTheme(document.documentElement.classList.contains("dark") ? "dark" : "light");

    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onSystemChange = () => {
      if (localStorage.getItem(STORAGE_KEY)) return;
      const next: Theme = media.matches ? "dark" : "light";
      applyTheme(next);
      setTheme(next);
    };
    media.addEventListener("change", onSystemChange);
    return () => media.removeEventListener("change", onSystemChange);
  }, []);

  const choose = (next: Theme) => {
    localStorage.setItem(STORAGE_KEY, next);
    applyTheme(next);
    setTheme(next);
  };

  return (
    <div className="theme-toggle" role="group" aria-label="Color theme">
      <span aria-hidden="true" className={`theme-thumb${theme === "dark" ? " pos-dark" : ""}`} />
      <button
        type="button"
        className={`theme-opt${theme === "light" ? " on" : ""}`}
        aria-label="Switch to light theme"
        aria-pressed={theme === "light"}
        onClick={() => choose("light")}
      >
        <Sun aria-hidden="true" />
      </button>
      <button
        type="button"
        className={`theme-opt${theme === "dark" ? " on" : ""}`}
        aria-label="Switch to dark theme"
        aria-pressed={theme === "dark"}
        onClick={() => choose("dark")}
      >
        <Moon aria-hidden="true" />
      </button>
    </div>
  );
}
