import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { navLinks } from "./content";
import { BrandMark } from "./icons";

export function SiteNav() {
  const [active, setActive] = useState<string>("");
  const [mobileOpen, setMobileOpen] = useState(false);
  const [scrolled, setScrolled] = useState(false);
  const linkRefs = useRef(new Map<string, HTMLAnchorElement>());
  const indicatorRef = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    const sections = Array.from(document.querySelectorAll<HTMLElement>("section[id]"));
    let ticking = false;

    const measure = () => {
      ticking = false;
      const y = window.scrollY;
      let current = "";
      for (const sec of sections) {
        if (y >= sec.getBoundingClientRect().top + y - 140) current = sec.id;
      }
      setActive(current);
      setScrolled(y > 24);
    };

    // One measurement per frame at most — the scroll event only schedules work.
    const onScroll = () => {
      if (ticking) return;
      ticking = true;
      requestAnimationFrame(measure);
    };

    measure();
    window.addEventListener("scroll", onScroll, { passive: true });
    return () => window.removeEventListener("scroll", onScroll);
  }, []);

  // The active pill slides between links instead of swapping background.
  useLayoutEffect(() => {
    const indicator = indicatorRef.current;
    if (!indicator) return;
    const link = linkRefs.current.get(active);
    if (!link) {
      indicator.style.opacity = "0";
      return;
    }
    indicator.style.opacity = "1";
    indicator.style.width = `${link.offsetWidth}px`;
    indicator.style.transform = `translateX(${link.offsetLeft}px)`;
  }, [active]);

  useEffect(() => {
    const onResize = () => {
      const link = linkRefs.current.get(active);
      const indicator = indicatorRef.current;
      if (!link || !indicator) return;
      indicator.style.width = `${link.offsetWidth}px`;
      indicator.style.transform = `translateX(${link.offsetLeft}px)`;
    };
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, [active]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setMobileOpen(false);
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  const closeMobileMenu = () => setMobileOpen(false);

  return (
    <header className={`nav${scrolled ? " scrolled" : ""}`}>
      <a className="brand" href="#top">
        <BrandMark />
        <span className="brand-tx">
          <b>SyncIt</b>
        </span>
      </a>
      <nav aria-label="Primary navigation" className="nav-links">
        <span aria-hidden="true" className="nav-indicator" ref={indicatorRef} />
        {navLinks.map((l) => (
          <a
            key={l.id}
            ref={(el) => {
              if (el) linkRefs.current.set(l.id, el);
              else linkRefs.current.delete(l.id);
            }}
            aria-current={active === l.id ? "page" : undefined}
            className={`nav-link${active === l.id ? " on" : ""}`}
            href={`#${l.id}`}
          >
            {l.label}
          </a>
        ))}
      </nav>
      <button
        aria-controls="mobile-navigation"
        aria-expanded={mobileOpen}
        aria-label={mobileOpen ? "Close navigation" : "Open navigation"}
        className={`nav-burger${mobileOpen ? " open" : ""}`}
        onClick={() => setMobileOpen((open) => !open)}
        type="button"
      >
        <i />
        <i />
      </button>
      <nav
        aria-label="Mobile navigation"
        className={`nav-mobile${mobileOpen ? " open" : ""}`}
        id="mobile-navigation"
      >
        {navLinks.map((l) => (
          <a
            key={l.id}
            aria-current={active === l.id ? "page" : undefined}
            className={`nav-mobile-link${active === l.id ? " on" : ""}`}
            href={`#${l.id}`}
            onClick={closeMobileMenu}
          >
            <span>{l.label}</span>
            <span className="nav-mobile-num">
              {String(navLinks.indexOf(l) + 1).padStart(2, "0")}
            </span>
          </a>
        ))}
      </nav>
    </header>
  );
}
