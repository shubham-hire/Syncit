import { useEffect, useRef } from "react";

/**
 * Live demo of the sync problem the Rust core solves: the listener's waveform
 * drifts behind the host's, then a critically-damped spring correction snaps
 * it back onto the same beat. The two rows render the identical envelope, so
 * a horizontal shift is exactly the delay the listener hears.
 */

const BAR_COUNT = 52;

/** Deterministic pseudo-audio envelope, shared by both rows. */
const ENVELOPE = Array.from({ length: BAR_COUNT }, (_, i) => {
  const n = Math.abs(Math.sin(i * 12.9898 + 4.1) * 43758.5453) % 1;
  const swell = 0.55 + 0.45 * Math.sin(i * 0.42);
  return 0.18 + 0.82 * n * swell;
});

const MAX_OFFSET = 34; // px of drift ≈ the ms figure shown in the status

export function SyncDemo() {
  const listenerRef = useRef<HTMLDivElement>(null);
  const statusRef = useRef<HTMLParagraphElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;

    let raf = 0;
    let cancelled = false;
    let visible = false;
    let offset = 0;
    let velocity = 0;
    let phase: "drift" | "correct" | "hold" = "drift";
    let phaseT = 0;
    let last = 0;

    const io = new IntersectionObserver(
      (entries) => {
        const entry = entries[0];
        visible = !!entry?.isIntersecting;
        if (visible && !raf) {
          last = performance.now();
          raf = requestAnimationFrame(tick);
        }
      },
      { threshold: 0.2 },
    );
    if (panelRef.current) io.observe(panelRef.current);

    function tick(now: number) {
      raf = 0;
      if (cancelled) return;
      const dt = Math.min((now - last) / 1000, 0.05);
      last = now;
      phaseT += dt;

      if (phase === "drift") {
        offset = MAX_OFFSET * Math.min(phaseT / 2.1, 1);
        if (phaseT > 2.5) {
          phase = "correct";
          phaseT = 0;
        }
      } else if (phase === "correct") {
        // Critically damped spring: no overshoot, motion starts from the
        // current on-screen offset and decays onto the target.
        const k = 110;
        const c = 2 * Math.sqrt(k);
        const accel = -k * offset - c * velocity;
        velocity += accel * dt;
        offset += velocity * dt;
        if (phaseT > 1.3 || (Math.abs(offset) < 0.4 && Math.abs(velocity) < 2)) {
          phase = "hold";
          phaseT = 0;
          offset = 0;
          velocity = 0;
        }
      } else if (phaseT > 2.4) {
        phase = "drift";
        phaseT = 0;
        offset = 0;
      }

      if (listenerRef.current) {
        listenerRef.current.style.setProperty("--sync-offset", `${offset.toFixed(2)}px`);
      }
      if (statusRef.current) {
        const inSync = offset < 0.75;
        statusRef.current.textContent = inSync
          ? "In sync · 0ms"
          : `Listener ${Math.round(offset)}ms behind`;
        statusRef.current.dataset["state"] = inSync ? "ok" : "warn";
      }

      if (visible) raf = requestAnimationFrame(tick);
    }

    return () => {
      cancelled = true;
      if (raf) cancelAnimationFrame(raf);
      io.disconnect();
    };
  }, []);

  return (
    <div className="demo-panel" data-reveal="up" ref={panelRef}>
      <div className="demo-head">
        <p className="demo-title">Host vs. listener — the same signal, one clock apart.</p>
        <p className="demo-status" data-state="ok" ref={statusRef}>
          In sync · 0ms
        </p>
      </div>
      <div className="demo-rows" aria-hidden="true">
        <div className="demo-row">
          <span className="demo-lbl">Host</span>
          <div className="demo-strip">
            {ENVELOPE.map((h, i) => (
              <i key={i} style={{ transform: `scaleY(${h.toFixed(3)})` }} />
            ))}
          </div>
        </div>
        <div className="demo-row">
          <span className="demo-lbl">Listener</span>
          <div className="demo-strip is-listener" ref={listenerRef}>
            {ENVELOPE.map((h, i) => (
              <i key={i} style={{ transform: `scaleY(${h.toFixed(3)})` }} />
            ))}
          </div>
        </div>
      </div>
      <p className="demo-note">
        The listener&rsquo;s jitter buffer re-orders packets and schedules playback against a shared
        clock, so drift is corrected automatically — no one touches a dial.
      </p>
    </div>
  );
}
