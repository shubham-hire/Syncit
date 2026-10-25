import { createFileRoute } from "@tanstack/react-router";
import { KageScene } from "@/components/kage/KageScene";
import { SiteNav } from "@/components/kage/SiteNav";
import {
  FaqSection,
  FinaleSection,
  FlowsSection,
  GateSection,
  HallmarksSection,
  Hero,
  RoadmapSection,
  SiteFooter,
} from "@/components/kage/Sections";
import { useReveal } from "@/components/kage/use-reveal";
import { useTilt } from "@/components/kage/tilt";
import { ThemeToggle } from "@/components/kage/ThemeToggle";

const TITLE = "SyncIt — Local Audio Sharing Prototype";
const DESCRIPTION =
  "SyncIt is a local audio-sharing prototype for a Windows host and Android listeners using their own headphones over Wi-Fi.";

export const Route = createFileRoute("/")({
  head: () => ({
    meta: [
      { title: TITLE },
      { name: "description", content: DESCRIPTION },
      { property: "og:title", content: TITLE },
      { property: "og:description", content: DESCRIPTION },
      { property: "og:type", content: "website" },
      { name: "twitter:card", content: "summary_large_image" },
      { property: "og:image", content: "/og.png" },
      { name: "twitter:image", content: "/og.png" },
    ],
  }),
  component: Index,
});

function Index() {
  useReveal();
  useTilt();

  return (
    <>
      <KageScene />
      <div className="page" id="top">
        <a className="skip-link" href="#main-content">
          Skip to content
        </a>
        <SiteNav />
        <ThemeToggle />
        <main id="main-content">
          <Hero />
          <GateSection />
          <FlowsSection />
          <HallmarksSection />
          <RoadmapSection />
          <FaqSection />
          <FinaleSection />
        </main>
        <SiteFooter />
      </div>
    </>
  );
}
