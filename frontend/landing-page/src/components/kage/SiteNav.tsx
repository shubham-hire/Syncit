import { useEffect, useState } from "react";
import { navLinks } from "./content";
import { BrandMark } from "./icons";

export function SiteNav() {
  const [active, setActive] = useState<string>("");
  const [mobileOpen, setMobileOpen] = useState(false);
  const [scrolled, setScrolled] = useState(false);

  useEffect(() => {
    const onScroll = () => {
      const y = window.pageYOffset || document.documentElement.scrollTop;
      let current = "";
      document.querySelectorAll<HTMLElement>("section[id]").forEach((sec) => {
        const sectionTop = sec.getBoundingClientRect().top + y;
        if (y >= sectionTop - 140) current = sec.id;
      });
      setActive(current);
      setScrolled(y > 24);
    };

    onScroll();
    window.addEventListener("scroll", onScroll, { passive: true });
    return () => window.removeEventListener("scroll", onScroll);
  }, []);

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
          <b>SYNCIT</b>
          <i>LOCAL AUDIO SHARING</i>
        </span>
      </a>
      <nav aria-label="Primary navigation" className="nav-links">
        {navLinks.map((l) => (
          <a
            key={l.id}
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
