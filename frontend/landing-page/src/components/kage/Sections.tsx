import { useEffect, useRef } from "react";
import {
  downloadAction,
  faqs,
  flows,
  footerColumns,
  hallmarks,
  heroAction,
  roadmap,
  stats,
  statusPill,
  steps,
} from "./content";
import { SyncDemo } from "./sync-demo";
import { BrandMark, ChevronIcon, DownloadIcon } from "./icons";
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@/components/ui/accordion";

/** Counts the numeric tokens of a stat up from zero when scrolled into view. */
function StatValue({ value }: { value: string }) {
  const ref = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const tokens = value.match(/(\d+|\D+)/g) ?? [];
    if (!tokens.some((t) => /^\d+$/.test(t))) return; // word-only values stay static

    const io = new IntersectionObserver(
      (entries) => {
        const entry = entries[0];
        if (!entry?.isIntersecting) return;
        io.disconnect();
        const duration = 1200;
        const start = performance.now();
        const tick = (now: number) => {
          const p = Math.min((now - start) / duration, 1);
          const ease = 1 - Math.pow(1 - p, 4);
          el.textContent = tokens
            .map((t) => (/^\d+$/.test(t) ? String(Math.round(Number(t) * ease)) : t))
            .join("");
          if (p < 1) requestAnimationFrame(tick);
        };
        requestAnimationFrame(tick);
      },
      { threshold: 0.6 },
    );
    io.observe(el);
    return () => io.disconnect();
  }, [value]);

  return <span ref={ref}>{value}</span>;
}

function SectionIntro({
  eyebrow,
  dark,
  children,
}: {
  eyebrow: string;
  dark?: boolean;
  children: React.ReactNode;
}) {
  return (
    <div className="sec-intro" data-reveal="up">
      <span className={`eyebrow${dark ? " eyebrow-dark" : ""}`}>{eyebrow}</span>
      <h2 className="display h-sec">{children}</h2>
    </div>
  );
}

export function Hero() {
  return (
    <section className="hero" id="hero">
      <div className="hero-grid">
        <div className="hero-copy">
          <div className="status-pill" data-reveal="up">
            <span className="pulse-dot" />
            <span>{statusPill}</span>
          </div>
          <h1 className="display h-hero" data-reveal="up" data-reveal-delay="90">
            One audio session.
            <br />
            Everyone listens <span className="grad">together.</span>
          </h1>
          <p className="hero-sub body-lg" data-reveal="up" data-reveal-delay="180">
            SyncIt shares what your PC is playing with everyone&rsquo;s own headphones over Wi-Fi —
            no internet, no pairing math, and every listener on the same beat.
          </p>
          <div className="hero-ctas" data-reveal="up" data-reveal-delay="270">
            <a
              className="btn btn-primary"
              href={downloadAction.href}
              rel="noreferrer"
              target="_blank"
            >
              <span>{downloadAction.label}</span>
              <DownloadIcon />
            </a>
            <a className="btn-link" href={heroAction.href}>
              <span>{heroAction.label}</span>
              <ChevronIcon />
            </a>
          </div>
        </div>

        <div className="hero-visual" aria-hidden="true">
          <div className="broadcast-carrier">
            <div className="carrier-wave" />
            <div className="carrier-wave" />
            <div className="carrier-wave" />
          </div>
        </div>
      </div>

      <div className="hero-cue">
        <span>Scroll to explore</span>
      </div>
    </section>
  );
}

export function GateSection() {
  return (
    <section className="sec" id="gate">
      <SectionIntro eyebrow="How it works">
        Three steps from one PC
        <br />
        to every headphones in the room.
      </SectionIntro>
      <div className="steps">
        {steps.map((s, i) => (
          <article
            className="step-card"
            key={s.num}
            data-reveal="up"
            data-tilt
            data-reveal-delay={String(i * 110)}
          >
            <span className="step-num">{s.num}</span>
            <h3>{s.title}</h3>
            <p>{s.text}</p>
          </article>
        ))}
      </div>
      <div className="stats" data-reveal="up">
        {stats.map((s) => (
          <div className="stat" key={s.label}>
            <b>
              <StatValue value={s.value} />
            </b>
            <span>{s.label}</span>
          </div>
        ))}
      </div>
    </section>
  );
}

function HostMock() {
  return (
    <div aria-hidden="true" className="mock mock-host">
      <div className="mock-bar">
        <i />
        <i />
        <i />
      </div>
      <div className="mock-code">
        <span className="mock-lbl">Session code</span>
        <span className="mock-code-val">
          <b>4</b>
          <b>8</b>
          <b>2</b>
          <b>6</b>
        </span>
      </div>
      <div className="mock-row">
        <span className="mock-avatar av-a">M</span>
        <span className="mock-name">
          <b>Meera</b>
          <i>Pixel · Bluetooth</i>
        </span>
        <span className="mock-pill ok">In sync</span>
      </div>
      <div className="mock-row">
        <span className="mock-avatar av-b">D</span>
        <span className="mock-name">
          <b>Dev</b>
          <i>Wired · 3.5mm</i>
        </span>
        <span className="mock-pill ok">In sync</span>
      </div>
      <div className="mock-row is-pending">
        <span className="mock-avatar av-c">S</span>
        <span className="mock-name">
          <b>Sam</b>
          <i>Waiting for approval</i>
        </span>
        <span className="mock-pill">Approve</span>
      </div>
    </div>
  );
}

function ListenerMock() {
  return (
    <div aria-hidden="true" className="mock mock-listener">
      <div className="mock-track">
        <span className="mock-art" />
        <span className="mock-name">
          <b>Now playing</b>
          <i>From the host · Local Wi-Fi</i>
        </span>
        <span className="mock-pill ok">Live</span>
      </div>
      <div className="mock-progress">
        <i />
      </div>
      <div className="mock-ctrl">
        <span className="mock-lbl">Delay</span>
        <span className="mock-slider">
          <i style={{ width: "34%" }} />
        </span>
        <b>+34ms</b>
      </div>
      <div className="mock-ctrl">
        <span className="mock-lbl">Volume</span>
        <span className="mock-slider">
          <i style={{ width: "72%" }} />
        </span>
        <b>72%</b>
      </div>
      <div className="mock-ctrl">
        <span className="mock-lbl">Mute</span>
        <span className="mock-switch">
          <i />
        </span>
        <b>Off</b>
      </div>
    </div>
  );
}

export function FlowsSection() {
  return (
    <section className="sec sec-fog" id="pathways">
      <SectionIntro eyebrow="Host + Listener">
        A control room for the host.
        <br />
        Personal controls for each listener.
      </SectionIntro>
      <div className="flows">
        {flows.map((f, i) => (
          <article
            className="flow-card"
            key={f.tag}
            data-reveal="up"
            data-tilt
            data-reveal-delay={String(i * 130)}
          >
            <div className="flow-stage">
              <span className="flow-tag">{f.tag}</span>
              {f.kind === "host" ? <HostMock /> : <ListenerMock />}
            </div>
            <div className="flow-copy">
              <h3>{f.title}</h3>
              <p>{f.text}</p>
              <div className="flow-meta">
                {f.meta.map((m) => (
                  <span key={m}>{m}</span>
                ))}
              </div>
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}

export function HallmarksSection() {
  return (
    <section className="sec sec-dark" id="lessons">
      <SectionIntro dark eyebrow="Sync features">
        The engineering behind
        <br />
        staying on the same beat.
      </SectionIntro>
      <SyncDemo />
      <div className="bento">
        {hallmarks.map((h, i) => (
          <article
            className={`bento-card${h.span === "wide" ? " is-wide" : ""}`}
            key={h.title}
            data-reveal="up"
            data-tilt
            data-reveal-delay={String(i * 90)}
          >
            <span className="bento-num">{String(i + 1).padStart(2, "0")}</span>
            <h3>{h.title}</h3>
            <p>{h.text}</p>
            <span className="bento-tag">{h.tag}</span>
          </article>
        ))}
      </div>
    </section>
  );
}

export function RoadmapSection() {
  return (
    <section className="sec" id="roadmap">
      <SectionIntro eyebrow="Roadmap">Working today. Shipping next.</SectionIntro>
      <div className="roadmap">
        {roadmap.map((col, i) => (
          <div
            className="roadmap-col"
            key={col.phase}
            data-reveal="up"
            data-reveal-delay={String(i * 130)}
          >
            <h3>
              <i className={i === 0 ? "dot ok" : "dot"} />
              {col.phase}
            </h3>
            <ul>
              {col.items.map((item) => (
                <li key={item}>{item}</li>
              ))}
            </ul>
          </div>
        ))}
      </div>
    </section>
  );
}

export function FaqSection() {
  return (
    <section className="sec sec-fog" id="faq">
      <SectionIntro eyebrow="FAQ">
        Questions people ask
        <br />
        before they plug in.
      </SectionIntro>
      <div className="faq-list" data-reveal="up">
        <Accordion type="single" collapsible>
          {faqs.map((f, i) => (
            <AccordionItem key={f.q} value={`faq-${i}`}>
              <AccordionTrigger className="faq-q">{f.q}</AccordionTrigger>
              <AccordionContent>
                <div className="faq-a">{f.a}</div>
              </AccordionContent>
            </AccordionItem>
          ))}
        </Accordion>
      </div>
    </section>
  );
}

export function FinaleSection() {
  return (
    <section className="sec fin" id="eternity">
      <span className="eyebrow" data-reveal="up">
        Prototype
      </span>
      <h2 className="display h-fin" data-reveal="up" data-reveal-delay="90">
        Explore
        <br />
        <span className="grad">SyncIt.</span>
      </h2>
      <p className="body-lg" data-reveal="up" data-reveal-delay="190">
        The current build ships a Flutter interaction prototype, Rust packet and timing primitives,
        and an offline simulator. Real Windows capture, Opus transport, and Android background
        playback are the next integrations.
      </p>
      <a
        className="btn btn-primary btn-lg"
        href={downloadAction.href}
        rel="noreferrer"
        target="_blank"
        data-reveal="up"
        data-reveal-delay="300"
      >
        <span>{downloadAction.label}</span>
        <DownloadIcon />
      </a>
      <p className="download-note" data-reveal="up" data-reveal-delay="400">
        Latest Windows and Android builds are on GitHub Releases. The page updates as real audio
        integration lands.
      </p>
    </section>
  );
}

export function SiteFooter() {
  return (
    <footer className="foot">
      <div className="foot-grid">
        <div className="foot-brand" data-reveal="up">
          <BrandMark />
          <p>
            SyncIt is a student-built local audio-sharing prototype for small groups using their own
            devices and headphones.
          </p>
        </div>
        {footerColumns.map((col) => (
          <div key={col.title}>
            <h4>{col.title}</h4>
            <ul>
              {col.links.map((l) => (
                <li key={l.label}>
                  <a href={l.href}>{l.label}</a>
                </li>
              ))}
            </ul>
          </div>
        ))}
      </div>
      <div className="foot-base">
        <span>© 2026 SyncIt Project</span>
        <span>Windows host · Android listeners · Local Wi-Fi</span>
      </div>
    </footer>
  );
}
