import type { SVGProps } from "react";

/**
 * Inline SVG icon set (lucide-style strokes, 24-grid). No icon dependency:
 * chrome icons only, kept tiny and tree-shakeable by construction.
 */

function base(props: SVGProps<SVGSVGElement>) {
  return {
    width: 18,
    height: 18,
    viewBox: "0 0 24 24",
    fill: "none",
    stroke: "currentColor",
    strokeWidth: 1.75,
    strokeLinecap: "round" as const,
    strokeLinejoin: "round" as const,
    ...props,
  };
}

export function IconDashboard(props: SVGProps<SVGSVGElement>) {
  return (
    <svg {...base(props)}>
      <rect x="3" y="3" width="7" height="9" rx="1.5" />
      <rect x="14" y="3" width="7" height="5" rx="1.5" />
      <rect x="14" y="12" width="7" height="9" rx="1.5" />
      <rect x="3" y="16" width="7" height="5" rx="1.5" />
    </svg>
  );
}

export function IconProjects(props: SVGProps<SVGSVGElement>) {
  return (
    <svg {...base(props)}>
      <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z" />
      <path d="M3 13h18" />
    </svg>
  );
}

export function IconSessions(props: SVGProps<SVGSVGElement>) {
  return (
    <svg {...base(props)}>
      <path d="M4 5h16" />
      <path d="M4 12h10" />
      <path d="M4 19h16" />
      <circle cx="18" cy="12" r="2" />
    </svg>
  );
}

export function IconProfiles(props: SVGProps<SVGSVGElement>) {
  return (
    <svg {...base(props)}>
      <circle cx="9" cy="8" r="3.25" />
      <path d="M3.5 19c.6-2.8 2.8-4.5 5.5-4.5s4.9 1.7 5.5 4.5" />
      <path d="M17 8.5a2.75 2.75 0 1 1 0 5.5" />
      <path d="M18 14.5c2 .4 3.4 1.9 3.8 4" />
    </svg>
  );
}

export function IconUsage(props: SVGProps<SVGSVGElement>) {
  return (
    <svg {...base(props)}>
      <path d="M4 20V10" />
      <path d="M10 20V4" />
      <path d="M16 20v-7" />
      <path d="M21 20H3" />
    </svg>
  );
}

export function IconFleet(props: SVGProps<SVGSVGElement>) {
  return (
    <svg {...base(props)}>
      <rect x="3" y="4" width="18" height="8" rx="1.5" />
      <rect x="3" y="14" width="18" height="6" rx="1.5" />
      <path d="M7 8h.01M7 17h.01" />
    </svg>
  );
}

export function IconTerminal(props: SVGProps<SVGSVGElement>) {
  return (
    <svg {...base(props)}>
      <path d="m5 7 4 4-4 4" />
      <path d="M12 17h7" />
    </svg>
  );
}

export function IconJuggler(props: SVGProps<SVGSVGElement>) {
  // Mark: three balls mid-juggle over a dashed throw arc. The balls are
  // the three harnesses in flight (violet, bright violet, live cyan).
  return (
    <svg {...base(props)}>
      <path
        d="M6.8 16C8.4 7.4 15.6 7.4 17.2 16"
        stroke="currentColor"
        strokeOpacity={0.35}
        strokeWidth={1.25}
        strokeDasharray="2.2 2.6"
        fill="none"
      />
      <circle cx={6.8} cy={16} r={2.3} fill="var(--color-accent)" />
      <circle cx={12} cy={8.6} r={2.5} fill="var(--color-accent-hi)" />
      <circle cx={17.2} cy={16} r={2.3} fill="var(--color-live)" />
    </svg>
  );
}
