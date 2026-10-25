export const navLinks = [
  { id: "gate", label: "How it works" },
  { id: "pathways", label: "Host + Listener" },
  { id: "lessons", label: "Sync features" },
  { id: "eternity", label: "Prototype" },
] as const;

export const heroAction = {
  label: "See how it works",
  href: "#gate",
} as const;

export const downloadAction = {
  label: "Download SyncIt",
  href: "https://github.com/shubham-hire/Syncit/releases/latest",
} as const;

export const statusPill =
  "Prototype · mock host/listener flows, offline timing core — real audio next";

export const steps = [
  {
    num: "01",
    title: "Host starts a session",
    text: "A Windows machine becomes the host. It captures whatever is playing and opens a session on the local Wi-Fi network.",
  },
  {
    num: "02",
    title: "Listeners join with a code",
    text: "Friends on Android enter the session code. The host approves each listener — no accounts, no pairing, no internet.",
  },
  {
    num: "03",
    title: "Everyone hears it in sync",
    text: "Timestamped packets and per-listener delay tuning keep every pair of headphones on the same beat.",
  },
] as const;

export const flows = [
  {
    tag: "Host",
    title: "One host, in control.",
    text: "The host flow models a session code, a live listener list, and one-tap approve or remove — built for the person whose machine is playing the music.",
    meta: ["Windows first", "Session host"],
    kind: "host" as const,
  },
  {
    tag: "Listener",
    title: "Tuned to your ears.",
    text: "Each listener gets independent delay, volume, and mute controls — so wired and Bluetooth headphones can land on the same beat.",
    meta: ["Android target", "Wired or Bluetooth"],
    kind: "listener" as const,
  },
] as const;

type Hallmark = {
  title: string;
  text: string;
  tag: string;
  span?: "wide";
};

export const hallmarks: Hallmark[] = [
  {
    title: "Timestamped audio packets",
    text: "The Rust core encodes sequence numbers and timestamps into every frame, so a listener can reorder packets and schedule playback against a shared clock.",
    tag: "Packet format",
    span: "wide",
  },
  {
    title: "Clock offset estimation",
    text: "NTP-style samples estimate the offset between each listener's clock and the host, turning two independent clocks into one shared timeline.",
    tag: "Timing model",
    span: "wide",
  },
  {
    title: "Bounded jitter buffer",
    text: "Packets are reordered, memory stays bounded, and overdue frames are skipped deliberately.",
    tag: "Playback buffer",
  },
  {
    title: "Manual delay adjustment",
    text: "The UI models compensation for the latency gap between wired and Bluetooth output.",
    tag: "Per-listener control",
  },
  {
    title: "Background playback target",
    text: "Real Android playback is planned around a foreground service, so listening continues reliably with the screen locked.",
    tag: "Next integration",
    span: "wide",
  },
];

export const stats = [
  { value: "2–5", label: "Listeners per session" },
  { value: "Local", label: "Wi-Fi only — no internet" },
  { value: "20–50ms", label: "Sync target after calibration" },
  { value: "1", label: "Windows host to start" },
] as const;

export const footerColumns = [
  {
    title: "Product",
    links: [
      { label: "How it works", href: "#gate" },
      { label: "Host + listener flows", href: "#pathways" },
      { label: "Sync features", href: "#lessons" },
      { label: "Prototype status", href: "#eternity" },
    ],
  },
  {
    title: "Engineering",
    links: [
      { label: "Timestamped packets", href: "#lessons" },
      { label: "Clock estimation", href: "#lessons" },
      { label: "Jitter buffer", href: "#lessons" },
    ],
  },
  {
    title: "Project",
    links: [
      { label: "Local-first design", href: "#gate" },
      { label: "Windows + Android", href: "#pathways" },
      { label: "Download builds", href: "https://github.com/shubham-hire/Syncit/releases/latest" },
    ],
  },
] as const;
