import {
  downloadAction,
  footerColumns,
  hallmarks,
  heroAction,
  heroChapters,
  receivers,
} from "./content";
import { ArrowIcon, BrandMark, DownloadIcon } from "./icons";

function SectionHead({ num, title }: { num: string; title: string }) {
  return (
    <div className="sec-head" data-reveal="up">
      <h2 className="k">
        <b>{num}</b> — {title}
      </h2>
      <span className="rule-line" data-reveal="line" data-reveal-delay="160" />
    </div>
  );
}

export function Hero() {
  return (
    <section className="hero" id="hero">
      <div className="hero-body-wrap">
        <div className="hero-top">
          <div className="eyebrow">
            <span className="dot" /> Chapter 00 — Local Audio Sharing Prototype
          </div>
          <h1 className="display h-hero">
            One Audio Session.
            <br />
            Everyone Listens
            <br />
            Together.
          </h1>
          <p className="hero-sub body-copy">
            SyncIt explores how a host could share local audio with a small group over Wi-Fi. The
            current build demonstrates the interface and offline timing core; real capture,
            transport, and playback are next.
          </p>
          <div className="hero-sync-status">
            <span className="pulse-dot" />
            Current build: mock host/listener flows · offline timing core · real audio next
          </div>
          <a className="arrowlink hero-action" href={heroAction.href}>
            <span>{heroAction.label}</span>
            <span className="ar">
              <ArrowIcon />
            </span>
          </a>
          <a className="hero-download" href={downloadAction.href} rel="noreferrer" target="_blank">
            <span>{downloadAction.label}</span>
            <DownloadIcon />
          </a>
        </div>

        <div className="hero-model-container">
          <div className="broadcast-carrier">
            <div className="carrier-wave" />
            <div className="carrier-wave" />
            <div className="carrier-wave" />
          </div>
        </div>
      </div>

      <div className="hero-spacer" />

      <div className="hero-foot">
        <div className="hero-cue">
          <span>Scroll to see how it works</span>
          <span className="track">
            <i />
          </span>
        </div>
        <div className="chapters">
          {heroChapters.map((c) => (
            <div className="chip" key={c.num}>
              <span className="num">{c.num}</span>
              <span className="tx">
                <b>{c.title}</b>
                <p>{c.text}</p>
              </span>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

export function ProtocolSection() {
  return (
    <section className="sec" id="gate">
      <SectionHead num="01" title="How SyncIt Is Designed" />
      <div className="gate-grid">
        <h3 className="display h-sec" data-reveal="up">
          Share one local audio session without sharing headphones.
        </h3>
        <div className="gate-copy">
          <p className="lead" data-reveal="up">
            SyncIt is designed for small groups that want to hear the same audio while using their
            own devices. The intended first path is a Windows host sending timestamped audio over a
            local network to approved Android listeners.
          </p>
          <p className="body-copy" data-reveal="up" data-reveal-delay="110">
            The current UI accepts a mock session code and models approval and playback controls. QR
            support, network transport, and Android playback are planned integrations; internet
            access is not part of the intended core experience.
          </p>
          <a className="arrowlink" href="#pathways" data-reveal="up" data-reveal-delay="220">
            <span>See the host and listener flows</span>
            <span className="ar">
              <ArrowIcon />
            </span>
          </a>
        </div>
      </div>
      <div className="gate-stats">
        <div data-reveal="up">
          <b>2–5</b>
          <span className="lbl">Target listeners</span>
        </div>
        <div data-reveal="up" data-reveal-delay="90">
          <b>Local</b>
          <span className="lbl">Wi-Fi session</span>
        </div>
        <div data-reveal="up" data-reveal-delay="180">
          <b>
            20–50<small>ms</small>
          </b>
          <span className="lbl">PRD target after calibration</span>
        </div>
        <div data-reveal="up" data-reveal-delay="270">
          <b>1</b>
          <span className="lbl">Windows host first</span>
        </div>
      </div>
    </section>
  );
}

export function ReceiversSection() {
  return (
    <section className="sec" id="pathways">
      <SectionHead num="02" title="Host and Listener Flows" />
      <div className="cards-mosaic">
        {receivers.map((r, i) => (
          <article
            className="rcard"
            key={r.title}
            data-reveal="up"
            data-reveal-delay={String(i * 130)}
          >
            <div className="rcard-fr">
              <img alt={r.alt} loading="lazy" referrerPolicy="no-referrer" src={r.src} />
              <span className="rcard-tag">{r.tag}</span>
              <div className="rcard-scrim" />
              <div className="rcard-lab">
                <h3>{r.title}</h3>
                <p>{r.text}</p>
              </div>
            </div>
            <div className="rcard-meta">
              <span>{r.metaLeft}</span>
              <span>{r.metaRight}</span>
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}

export function HallmarksSection() {
  return (
    <section className="sec" id="lessons">
      <SectionHead num="03" title="The Pieces Behind Synchronized Listening" />
      <div className="cur-head">
        <h3 className="display h-sec" data-reveal="up">
          Model the path. Buffer the stream. Adjust per listener.
        </h3>
        <p className="body-lg" data-reveal="up" data-reveal-delay="120">
          SyncIt separates host capture, network jitter, and device output delay in its design so
          each part can be measured when the real integrations arrive.
        </p>
      </div>
      <div className="cur">
        {hallmarks.map((h, i) => (
          <div className="les" key={h.title} data-reveal="up" data-reveal-delay={String(i * 90)}>
            <span className="k">{String(i + 1).padStart(2, "0")}</span>
            <h3>{h.title}</h3>
            <p>{h.text}</p>
            <span className="t">{h.tag}</span>
            <i className="bar" />
          </div>
        ))}
      </div>
    </section>
  );
}

export function BundleSection() {
  return (
    <section className="sec fin" id="eternity">
      <div className="eyebrow" data-reveal="up">
        Chapter 04 — Try the Prototype
      </div>
      <h2 className="display" data-reveal="up" data-reveal-delay="90">
        Explore
        <br />
        SyncIt
      </h2>
      <p className="body-lg" data-reveal="up" data-reveal-delay="190">
        The current build includes a Flutter interaction prototype, Rust packet and timing
        primitives, and an offline simulator. Real Windows capture, Opus transport, and Android
        background playback are the next integration steps.
      </p>
      <a
        className="cta"
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
        Download the latest Windows or Android build from GitHub Releases. The release page will be
        updated as the real audio integration becomes available.
      </p>
    </section>
  );
}

export function SiteFooter() {
  return (
    <footer className="foot">
      <div className="foot-grid">
        <div className="foot-brand" data-reveal="up">
          <BrandMark withBase={false} />
          <p>
            SyncIt is a student-built local audio-sharing prototype for small groups using their own
            devices and headphones.
          </p>
        </div>
        {footerColumns.map((col, i) => (
          <div key={col.title} data-reveal="up" data-reveal-delay={String(i * 100)}>
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
