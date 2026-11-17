/**
 * The few icons Z-I-P needs, drawn at 16px on a 4px grid with 1.5px strokes,
 * after Camu's set. Inline SVG: no icon font, no dependency.
 */

import type { JobState } from "../lib/model";
import { percent } from "../lib/model";

type IconProps = { className?: string };

const svg = (children: React.ReactNode, { className }: IconProps = {}) => (
  <svg className={className ?? "i"} width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden>
    {children}
  </svg>
);

const stroke = { stroke: "currentColor", strokeWidth: 1.5, strokeLinecap: "round" as const, strokeLinejoin: "round" as const };

export const Tray = (p: IconProps) =>
  svg(
    <>
      <path d="M2.5 9.5 4 3.75h8l1.5 5.75v2.75a.75.75 0 0 1-.75.75h-9.5a.75.75 0 0 1-.75-.75V9.5Z" {...stroke} />
      <path d="M2.5 9.5h3l.75 1.5h3.5l.75-1.5h3" {...stroke} />
    </>,
    p,
  );
export const History = (p: IconProps) => svg(<path d="M2.8 8a5.2 5.2 0 1 0 1.6-3.8M2.8 3v2.6h2.6M8 5.2V8l2 1.4" {...stroke} />, p);
export const Gear = (p: IconProps) =>
  svg(
    <>
      <circle cx="8" cy="8" r="2" {...stroke} />
      <path d="M8 2.25v1.5M8 12.25v1.5M13.75 8h-1.5M3.75 8h-1.5M12.07 3.93l-1.06 1.06M4.99 11.01l-1.06 1.06M12.07 12.07l-1.06-1.06M4.99 4.99 3.93 3.93" {...stroke} />
    </>,
    p,
  );
export const Plus = (p: IconProps) => svg(<path d="M8 3.5v9M3.5 8h9" {...stroke} />, p);
export const Folder = (p: IconProps) =>
  svg(<path d="M2.5 4.5v7.25c0 .41.34.75.75.75h9.5c.41 0 .75-.34.75-.75V6.25a.75.75 0 0 0-.75-.75H8L6.5 3.75H3.25a.75.75 0 0 0-.75.75Z" {...stroke} />, p);
export const File = (p: IconProps) =>
  svg(
    <>
      <path d="M4 2.75h5l3 3v7.5H4V2.75Z" {...stroke} />
      <path d="M9 2.75v3h3" {...stroke} />
    </>,
    p,
  );
export const Lock = (p: IconProps) =>
  svg(
    <>
      <rect x="3.5" y="7" width="9" height="6.25" rx="1.25" {...stroke} />
      <path d="M5.5 7V5.25a2.5 2.5 0 0 1 5 0V7" {...stroke} />
    </>,
    p,
  );
export const Close = (p: IconProps) => svg(<path d="m4.5 4.5 7 7M11.5 4.5l-7 7" {...stroke} />, p);
export const Search = (p: IconProps) =>
  svg(
    <>
      <circle cx="7" cy="7" r="4.25" {...stroke} />
      <path d="m10.25 10.25 3 3" {...stroke} />
    </>,
    p,
  );
export const Reveal = (p: IconProps) => svg(<path d="M6.5 3.5h-3v9h9v-3M9 3.5h3.5V7M12.5 3.5 7.5 8.5" {...stroke} />, p);
export const Retry = (p: IconProps) =>
  svg(<path d="M12.75 6.5A5 5 0 0 0 3.6 5.25M3.25 9.5a5 5 0 0 0 9.15 1.25M3.5 2.75v2.75h2.75M12.5 13.25V10.5H9.75" {...stroke} />, p);
export const Stop = (p: IconProps) =>
  svg(
    <>
      <circle cx="8" cy="8" r="6" {...stroke} />
      <rect x="6" y="6" width="4" height="4" rx=".5" fill="currentColor" />
    </>,
    p,
  );
export const CheckCircle = (p: IconProps) =>
  svg(
    <>
      <circle cx="8" cy="8" r="6" {...stroke} />
      <path d="m5.5 8.25 1.75 1.75 3.25-3.5" {...stroke} />
    </>,
    p,
  );
export const Warning = (p: IconProps) =>
  svg(
    <>
      <path d="M8 2.75 13.75 13H2.25L8 2.75Z" {...stroke} />
      <path d="M8 6.75v2.5M8 11.1v.15" {...stroke} />
    </>,
    p,
  );
export const Shield = (p: IconProps) =>
  svg(<path d="M8 2.5 3.5 4.25v3.5c0 2.75 1.9 4.75 4.5 5.75 2.6-1 4.5-3 4.5-5.75v-3.5L8 2.5Z" {...stroke} />, p);

/** The Z-I-P mark: a zipper's teeth closing into a pull tab. */
export function Logo({ size = 18 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden className="logo">
      <rect x="1" y="1" width="22" height="22" rx="6" fill="var(--accent)" />
      <path d="M12 4v10" stroke="#fff" strokeWidth="1.6" strokeDasharray="1.6 1.4" />
      <path d="M9.5 6h2M12.5 8h2M9.5 10h2M12.5 12h2" stroke="#fff" strokeWidth="1.4" strokeLinecap="round" />
      <rect x="9.25" y="14" width="5.5" height="6" rx="1.5" fill="#fff" />
      <circle cx="12" cy="17.75" r="1" fill="var(--accent)" />
    </svg>
  );
}

/**
 * The glyph at the start of each row, like an issue's status circle: a
 * ring that fills while unpacking, a tick when done, a cross when not.
 */
export function StatusGlyph({ state }: { state: JobState }) {
  switch (state.phase) {
    case "done":
      return <CheckCircle className="i s-done" />;
    case "failed":
      return svg(
        <>
          <circle cx="8" cy="8" r="6" {...stroke} />
          <path d="m6 6 4 4m0-4-4 4" {...stroke} />
        </>,
        { className: "i s-failed" },
      );
    case "cancelled":
      return <Stop className="i s-other" />;
    case "needsPassword":
      return <Lock className="i s-attention" />;
    case "browsing":
    case "waiting":
      return svg(<circle cx="8" cy="8" r="6" {...stroke} strokeDasharray="2 2.2" />, { className: "i s-other" });
    case "extracting": {
      const p = percent(state.done, state.total);
      const r = 5.5;
      const c = 2 * Math.PI * r;
      return (
        <svg className="i s-busy" width="16" height="16" viewBox="0 0 16 16" aria-hidden>
          <circle cx="8" cy="8" r={r} stroke="var(--line-strong)" strokeWidth="2" fill="none" />
          <circle
            className={p === null ? "spin" : undefined}
            cx="8"
            cy="8"
            r={r}
            stroke="currentColor"
            strokeWidth="2"
            fill="none"
            strokeLinecap="round"
            strokeDasharray={`${p === null ? c * 0.3 : Math.max((c * p) / 100, 1)} ${c}`}
            transform="rotate(-90 8 8)"
          />
        </svg>
      );
    }
    default:
      return <span className="i skeleton-dot" />;
  }
}
