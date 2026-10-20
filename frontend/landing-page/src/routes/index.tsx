import { createFileRoute } from "@tanstack/react-router";
import { KageScene } from "@/components/kage/KageScene";
import { SiteNav } from "@/components/kage/SiteNav";
import {
  BundleSection,
  HallmarksSection,
  Hero,
  ProtocolSection,
  ReceiversSection,
  SiteFooter,
} from "@/components/kage/Sections";

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
    ],
  }),
  component: Index,
});

function Index() {
  return (
    <>
      <KageScene />
      <div className="page" id="top">
        <a className="skip-link" href="#main-content">
          Skip to content
        </a>
        <SiteNav />
        <main id="main-content">
          <Hero />
          <ProtocolSection />
          <ReceiversSection />
          <HallmarksSection />
          <BundleSection />
        </main>
        <SiteFooter />
      </div>
    </>
  );
}
