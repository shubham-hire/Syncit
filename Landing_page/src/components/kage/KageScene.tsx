import { useEffect, useRef } from "react";

/** Fixed full-screen WebGL layer. three.js is loaded only in the browser. */
export function KageScene() {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;

    let cleanup: (() => void) | undefined;
    let cancelled = false;

    Promise.all([import("three"), import("./headphone-scene")])
      .then(([THREE, { mountHeadphoneScene }]) => {
        if (cancelled || !ref.current) return;
        try {
          cleanup = mountHeadphoneScene(ref.current, THREE);
        } catch (err) {
          console.warn("3D scene unavailable:", err);
        }
      })
      .catch((err) => console.warn("3D scene failed to load:", err));

    return () => {
      cancelled = true;
      cleanup?.();
    };
  }, []);

  return <div ref={ref} className="kg-scene" aria-hidden="true" />;
}
