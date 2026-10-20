export const navLinks = [
  { id: "gate", label: "How It Works" },
  { id: "pathways", label: "Host + Listener" },
  { id: "lessons", label: "Sync Features" },
  { id: "eternity", label: "Prototype" },
] as const;

export const heroAction = {
  label: "See the host and listener flow",
  href: "#pathways",
} as const;

export const downloadAction = {
  label: "Download SyncIt",
  href: "https://github.com/shubham-hire/Syncit/releases/latest",
} as const;

export const heroChapters = [
  {
    num: "01",
    title: "One host, several listeners.",
    text: "The target flow is one Windows host and two to five Android listeners.",
  },
  {
    num: "02",
    title: "Local-first by design.",
    text: "The planned session uses the host's Wi-Fi network without requiring internet access.",
  },
  {
    num: "03",
    title: "Personal playback controls.",
    text: "The Flutter UI prototype includes delay, volume, and mute controls per listener.",
  },
  {
    num: "04",
    title: "A measured first step.",
    text: "The Rust core and offline simulator come before real capture and playback integration.",
  },
];

export const receivers = [
  {
    tag: "Host",
    alt: "Audio equipment for a shared listening session",
    src: "https://lh3.googleusercontent.com/aida/AEtjO1V2beej4lV71lW2SNXwI3590DwyA9Gf_0d6o-6dOt9Cnr5LQ0uR73swN2mXFQ5BOCgwhmWMDC3TZJB3CA0cgyw_AkVXWifWBYwwb5Id_ZT_A-FbfPSc_wZ5YRafubuTWK8AWvU9ZxRzRbBuf1GIq5J_JNEQyTOGcs8NYHXSNmemDoExxSrHJyKdzq_n-8ki3Bd_KM3b7mUzFFT-cL0AWJsgqGWYHQHgitFkb7RdENLy0ixjwHutnK08fbo",
    title: "Prototype the host flow",
    text: "The current Flutter flow models a host starting a session, showing a code, and approving or removing listeners.",
    metaLeft: "Windows first",
    metaRight: "Session host",
  },
  {
    tag: "Listener",
    alt: "Wireless earbuds for a listener",
    src: "https://lh3.googleusercontent.com/aida/AEtjO1XA07ZqiCgz-z6eW0K0pzlI1Ciqgf6y2FdBX9DFeKmZDN9rUMe_i0T1z6FOhPYmQwTNfR00hOIeKmbCNtPyNGqGWQ_hn03vtGv2cxErAuG73mqTIt337iUuunBpIWyKILMNi_RR8UXFMium96o31iXa1ddxQEhrf2KUG8G_PCjsblKLwhnxv-2tAqp_FzKnVTtCLt0DDbCz5ewCkozEw6IKwQPWDEbJAjKxV9Dw7RI7BgQlM1JswHefO5E",
    title: "Plan the listener flow",
    text: "The current UI accepts a mock session code. Android playback and QR generation or scanning are planned next steps.",
    metaLeft: "Android target",
    metaRight: "Wired or Bluetooth",
  },
  {
    tag: "Personal controls",
    alt: "Headphones for a shared listening session",
    src: "https://lh3.googleusercontent.com/aida/AEtjO1VtrMew7MbP5N-SmvveYUbAB5_vckqRZiG5S5OqlTvuAu-t1qlqvqbIl1ZXZ9zK-QDmXCjhrIWFkpZfDs8TDQohv-gxTmfAr0t-IYeHubR21wBZVnKkK12Qj9dQugw1tv8GZIoCfP3aTt77RUWtkhkGmzAdkV_1D0KAZBUMNonWFipGvum2OWg_XNQES7fFZiCxmnOMuqpcALJ9t_jmb_Ic8r4N4Z5Q_iCoN10sG8M8-68GjM0DteAa2W-m",
    title: "Tune the mock playback",
    text: "The prototype exposes independent delay, volume, and mute controls to model output differences.",
    metaLeft: "Independent delay",
    metaRight: "Volume + mute",
  },
];

export const hallmarks = [
  {
    title: "Timestamped Audio Packets",
    text: "The Rust core encodes sequence numbers and timestamps so a future listener can reorder frames and schedule playback.",
    tag: "Packet Format",
  },
  {
    title: "Clock Offset Estimation",
    text: "The Rust core models NTP-style samples for estimating the offset between host and listener clocks.",
    tag: "Timing Model",
  },
  {
    title: "Bounded Jitter Buffer",
    text: "The tested Rust primitive reorders packets, bounds memory, and lets a future scheduler skip overdue frames deliberately.",
    tag: "Playback Buffer",
  },
  {
    title: "Manual Delay Adjustment",
    text: "The Flutter mock UI models compensation for differences between wired and Bluetooth output latency.",
    tag: "Per-Listener Control",
  },
  {
    title: "Background Playback Target",
    text: "Real Android playback is planned around a foreground service so screen-locked listening can continue reliably.",
    tag: "Next Integration Step",
  },
];

export const footerColumns = [
  {
    title: "Product",
    links: [
      { label: "How SyncIt works", href: "#gate" },
      { label: "Host flow", href: "#pathways" },
      { label: "Listener flow", href: "#pathways" },
      { label: "Prototype status", href: "#eternity" },
    ],
  },
  {
    title: "Engineering",
    links: [
      { label: "Timestamped packets", href: "#lessons" },
      { label: "Clock estimation", href: "#lessons" },
      { label: "Jitter buffer", href: "#lessons" },
      { label: "Manual calibration", href: "#lessons" },
    ],
  },
  {
    title: "Project",
    links: [
      { label: "Local-first design", href: "#gate" },
      { label: "Windows + Android", href: "#pathways" },
      { label: "Measured synchronization", href: "#lessons" },
    ],
  },
];
