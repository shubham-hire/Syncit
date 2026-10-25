import { useEffect } from "react";

/**
 * Spring-based pointer tilt for cards marked with `data-tilt`.
 *
 * Motion follows the apple-design rules: targets are set by the pointer,
 * the transform is driven by a critically damped spring (no overshoot)
 * that starts from the current on-screen rotation — so entering, reversing,
 * and leaving all blend without a jump. Only attaches on devices with a
 * fine hover-capable pointer, and stays out of reduced-motion.
 */
export function useTilt() {
  useEffect(() => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    if (!window.matchMedia("(hover: hover) and (pointer: fine)").matches) return;

    const MAX_TILT = 3; // deg
    const MAX_LIFT = 5; // px
    const stiffness = 170;
    const damping = 2 * Math.sqrt(stiffness); // critical damping

    const cleanups: Array<() => void> = [];

    for (const el of document.querySelectorAll<HTMLElement>("[data-tilt]")) {
      let rx = 0;
      let ry = 0;
      let vx = 0;
      let vy = 0;
      let trx = 0;
      let tryR = 0;
      let raf = 0;
      let last = 0;

      const apply = () => {
        el.style.transform = `perspective(900px) rotateX(${rx.toFixed(3)}deg) rotateY(${ry.toFixed(
          3,
        )}deg) translateY(${(-MAX_LIFT * ((Math.abs(rx) + Math.abs(ry)) / (2 * MAX_TILT))).toFixed(
          2,
        )}px)`;
      };

      const tick = (now: number) => {
        raf = 0;
        const dt = Math.min((now - last) / 1000, 0.05);
        last = now;
        vx += (-stiffness * rx - damping * vx) * dt;
        vy += (-stiffness * ry - damping * vy) * dt;
        rx += vx * dt;
        ry += vy * dt;
        apply();
        const settled =
          Math.abs(rx - trx) < 0.01 &&
          Math.abs(ry - tryR) < 0.01 &&
          Math.abs(vx) < 0.01 &&
          Math.abs(vy) < 0.01;
        if (settled) {
          rx = trx;
          ry = tryR;
          vx = 0;
          vy = 0;
          apply();
          return; // loop sleeps; next pointer event restarts it
        }
        raf = requestAnimationFrame(tick);
      };

      const start = () => {
        if (!raf) {
          last = performance.now();
          raf = requestAnimationFrame(tick);
        }
      };

      const onMove = (e: PointerEvent) => {
        // Skip while the scroll-reveal has not finished (it owns transform).
        if (el.style.opacity === "0") return;
        const rect = el.getBoundingClientRect();
        const nx = ((e.clientX - rect.left) / rect.width) * 2 - 1;
        const ny = ((e.clientY - rect.top) / rect.height) * 2 - 1;
        tryR = Math.max(Math.min(nx * MAX_TILT, MAX_TILT), -MAX_TILT);
        trx = Math.max(Math.min(-ny * MAX_TILT, MAX_TILT), -MAX_TILT);
        start();
      };

      const onLeave = () => {
        trx = 0;
        tryR = 0;
        start();
      };

      el.addEventListener("pointermove", onMove);
      el.addEventListener("pointerleave", onLeave);
      cleanups.push(() => {
        el.removeEventListener("pointermove", onMove);
        el.removeEventListener("pointerleave", onLeave);
        if (raf) cancelAnimationFrame(raf);
        el.style.transform = "";
      });
    }

    return () => cleanups.forEach((fn) => fn());
  }, []);
}
