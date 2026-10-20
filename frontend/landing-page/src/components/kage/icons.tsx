export function BrandMark({ size = 34, withBase = true }: { size?: number; withBase?: boolean }) {
  return (
    <svg aria-hidden="true" fill="none" viewBox="0 0 44 44" width={size} height={size}>
      <circle className="mark-seal" cx="22" cy="25" r="8.6" />
      <path className="mark-ink" d="M5 13h34M9 18.4h26M22 8.5v27" strokeWidth="1.8" />
      {withBase && (
        <path className="mark-ink" d="M14 35.5h16" strokeOpacity=".6" strokeWidth="1.4" />
      )}
    </svg>
  );
}

export function ArrowIcon() {
  return (
    <svg aria-hidden="true" fill="none" height="13" viewBox="0 0 14 14" width="13">
      <path d="M3 11 11 3M5 3h6v6" stroke="currentColor" strokeWidth="1.3" />
    </svg>
  );
}

export function DownloadIcon() {
  return (
    <svg aria-hidden="true" fill="none" height="14" viewBox="0 0 14 14" width="14">
      <path
        d="M7 2v7M4.5 6.5 7 9l2.5-2.5M3 11.5h8"
        stroke="currentColor"
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth="1.3"
      />
    </svg>
  );
}
