import { useEffect } from "react";

/**
 * One-shot scroll-reveal choreography.
 *
 * Elements opt in with `data-reveal="up"` (rise + fade) or `data-reveal="line"`
 * (horizontal rule draw), optionally with `data-reveal-delay` in milliseconds.
 * Initial hidden styles are applied inline from JS so a no-JS or reduced-motion
 * render stays fully visible; observed elements reveal once, then unobserve.
 */
export function useReveal() {
  useEffect(() => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;

    const targets = Array.from(document.querySelectorAll<HTMLElement>("[data-reveal]"));
    if (targets.length === 0) return;

    for (const el of targets) {
      const delay = Number(el.dataset["revealDelay"] ?? "") || 0;
      el.style.opacity = "0";
      if (el.dataset["reveal"] === "line") {
        el.style.transform = "scaleX(0)";
        el.style.transformOrigin = "left center";
      } else {
        el.style.transform = "translateY(28px)";
      }
      el.style.transition = [
        `opacity 0.9s var(--ease-out) ${delay}ms`,
        `transform 0.9s var(--ease-out) ${delay}ms`,
      ].join(", ");
      el.style.willChange = "opacity, transform";
    }

    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (!entry.isIntersecting) continue;
          const el = entry.target as HTMLElement;
          // Wait one frame so the hidden state is committed and the
          // transition actually runs instead of snapping.
          requestAnimationFrame(() => {
            el.style.opacity = "1";
            el.style.transform = "none";
            el.style.willChange = "";
          });
          observer.unobserve(el);
        }
      },
      { threshold: 0.12, rootMargin: "0px 0px -8% 0px" },
    );
    targets.forEach((el) => observer.observe(el));

    // Retire the hero scroll cue once the visitor starts moving.
    const heroCue = document.querySelector<HTMLElement>(".hero-cue");
    if (heroCue) heroCue.style.transition = "opacity 0.5s ease";
    const onScroll = () => {
      if (!heroCue) return;
      const y = window.pageYOffset || document.documentElement.scrollTop;
      heroCue.style.opacity = y > 60 ? "0" : "1";
    };
    window.addEventListener("scroll", onScroll, { passive: true });

    return () => {
      observer.disconnect();
      window.removeEventListener("scroll", onScroll);
    };
  }, []);
}
