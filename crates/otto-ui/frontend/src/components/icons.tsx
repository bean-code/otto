// Otto's own hand-authored outline icon set (24x24 viewBox, currentColor
// stroke) - ported 1:1 from the original vanilla-JS `ICONS` map. Kept
// separate from `lucide-react` (used by shadcn's own primitives internally)
// since these are Otto's distinctive branding icons, not generic UI chrome.

const PATHS = {
  close: <path d="M6 6l12 12M18 6L6 18" />,
  sparkle: <path d="M12 3l1.5 5.5L19 10l-5.5 1.5L12 17l-1.5-5.5L5 10l5.5-1.5L12 3Z" />,
  layers: (
    <>
      <path d="M12 3l8 4-8 4-8-4 8-4Z" />
      <path d="M4 12l8 4 8-4" />
      <path d="M4 16l8 4 8-4" />
    </>
  ),
  bolt: <path d="M13 2 4 14h6l-1 8 9-12h-6l1-8Z" />,
  globe: (
    <>
      <circle cx={12} cy={12} r={9} />
      <path d="M3 12h18" />
      <path d="M12 3c2.5 2.5 4 6 4 9s-1.5 6.5-4 9c-2.5-2.5-4-6-4-9s1.5-6.5 4-9Z" />
    </>
  ),
  branch: (
    <>
      <circle cx={6} cy={5} r={2} />
      <circle cx={6} cy={19} r={2} />
      <circle cx={18} cy={12} r={2} />
      <path d="M6 7v10" />
      <path d="M6 12h6a4 4 0 0 0 4-4" />
    </>
  ),
  trendUp: (
    <>
      <path d="M4 16l6-6 4 4 6-8" />
      <path d="M14 6h6v6" />
    </>
  ),
  calendar: (
    <>
      <rect x={3.5} y={5} width={17} height={16} rx={2} />
      <path d="M3.5 10h17" />
      <path d="M8 3v4" />
      <path d="M16 3v4" />
    </>
  ),
  bell: (
    <>
      <path d="M6 8a6 6 0 0 1 12 0c0 5 2 6 2 6H4s2-1 2-6" />
      <path d="M9.5 18a2.5 2.5 0 0 0 5 0" />
    </>
  ),
  code: (
    <>
      <path d="M8 6l-5 6 5 6" />
      <path d="M16 6l5 6-5 6" />
    </>
  ),
  gear: (
    <>
      <circle cx={12} cy={12} r={3} />
      <path d="M12 2.5v3M12 18.5v3M4.2 6.2l2.1 2.1M17.7 15.7l2.1 2.1M2.5 12h3M18.5 12h3M4.2 17.8l2.1-2.1M17.7 8.3l2.1-2.1" />
    </>
  ),
  settings: (
    <>
      <path d="M4 7h10M17 7h3" />
      <circle cx={14} cy={7} r={2} />
      <path d="M4 12h4M11 12h9" />
      <circle cx={8} cy={12} r={2} />
      <path d="M4 17h10M17 17h3" />
      <circle cx={14} cy={17} r={2} />
    </>
  ),
  mic: (
    <>
      <rect x={9} y={3} width={6} height={11} rx={3} />
      <path d="M5 11a7 7 0 0 0 14 0" />
      <path d="M12 18v3" />
      <path d="M8.5 21h7" />
    </>
  ),
  deploy: (
    <>
      <path d="M4 16l6-6 4 4 6-8" />
      <path d="M14 6h6v6" />
    </>
  ),
} as const;

export type IconName = keyof typeof PATHS;

export function Icon({ name, className = "w-5 h-5" }: { name: IconName; className?: string }) {
  return (
    <svg
      className={className}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.75}
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      {PATHS[name] ?? PATHS.gear}
    </svg>
  );
}
