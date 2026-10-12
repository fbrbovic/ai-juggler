import type { ButtonHTMLAttributes, HTMLAttributes, ReactNode } from "react";

/**
 * Base UI primitives, shadcn-style: hand-vendored, token-driven, no
 * runtime dependencies. Screen-level components compose from these.
 */

type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";
type ButtonSize = "sm" | "md";

const buttonVariants: Record<ButtonVariant, string> = {
  primary:
    "bg-accent text-white hover:bg-accent-hi active:translate-y-px shadow-[0_1px_0_rgba(255,255,255,0.15)_inset]",
  secondary:
    "bg-surface-2 text-ink border border-hairline hover:bg-surface-3 active:translate-y-px",
  ghost: "text-muted hover:text-ink hover:bg-surface-2",
  danger: "bg-danger/15 text-danger border border-danger/30 hover:bg-danger/25",
};

const buttonSizes: Record<ButtonSize, string> = {
  sm: "h-7 px-2.5 text-xs gap-1.5 rounded-lg",
  md: "h-9 px-3.5 text-sm gap-2 rounded-xl",
};

export function Button({
  variant = "secondary",
  size = "md",
  className = "",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: ButtonVariant;
  size?: ButtonSize;
}) {
  return (
    <button
      className={`inline-flex items-center justify-center font-medium transition-colors
        disabled:opacity-40 disabled:pointer-events-none cursor-default select-none
        ${buttonVariants[variant]} ${buttonSizes[size]} ${className}`}
      {...props}
    />
  );
}

export function Card({
  className = "",
  children,
  ...props
}: HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={`rounded-card border border-hairline bg-surface
        shadow-[0_1px_2px_rgba(0,0,0,0.35)] ${className}`}
      {...props}
    >
      {children}
    </div>
  );
}

export type ChipTone = "neutral" | "success" | "warning" | "danger" | "live" | "accent";

const chipTones: Record<ChipTone, string> = {
  neutral: "bg-surface-2 text-muted border-hairline",
  success: "bg-success-soft text-success border-success/25",
  warning: "bg-warning-soft text-warning border-warning/25",
  danger: "bg-danger-soft text-danger border-danger/25",
  live: "bg-live-soft text-live border-live/25",
  accent: "bg-accent-soft text-accent-hi border-accent/25",
};

export function Chip({
  tone = "neutral",
  dot = false,
  children,
}: {
  tone?: ChipTone;
  dot?: boolean;
  children: ReactNode;
}) {
  return (
    <span
      className={`inline-flex items-center gap-1.5 rounded-full border px-2 py-0.5
        text-[11px] font-medium tracking-wide ${chipTones[tone]}`}
    >
      {dot && <span className="size-1.5 rounded-full bg-current" />}
      {children}
    </span>
  );
}

export function StatCard({
  label,
  value,
  hint,
  tone = "neutral",
}: {
  label: string;
  value: string;
  hint?: string;
  tone?: ChipTone;
}) {
  const valueTone =
    tone === "live"
      ? "text-live"
      : tone === "success"
        ? "text-success"
        : tone === "warning"
          ? "text-warning"
          : "text-ink";
  return (
    <Card className="p-4 flex flex-col gap-1">
      <span className="text-[11px] uppercase tracking-widest text-faint">
        {label}
      </span>
      <span className={`text-2xl font-semibold tabular-nums ${valueTone}`}>
        {value}
      </span>
      {hint && <span className="text-xs text-muted">{hint}</span>}
    </Card>
  );
}

/** Circular progress ring for quota windows. */
export function QuotaRing({
  percent,
  size = 56,
  label,
}: {
  percent: number;
  size?: number;
  label?: string;
}) {
  const clamped = Math.max(0, Math.min(100, percent));
  const stroke = 5;
  const radius = (size - stroke) / 2;
  const circumference = 2 * Math.PI * radius;
  const color =
    clamped >= 90 ? "var(--color-danger)" : clamped >= 70 ? "var(--color-warning)" : "var(--color-accent)";
  return (
    <div className="flex flex-col items-center gap-1">
      <svg width={size} height={size} className="-rotate-90">
        <circle
          cx={size / 2}
          cy={size / 2}
          r={radius}
          fill="none"
          stroke="var(--color-hairline)"
          strokeWidth={stroke}
        />
        <circle
          cx={size / 2}
          cy={size / 2}
          r={radius}
          fill="none"
          stroke={color}
          strokeWidth={stroke}
          strokeLinecap="round"
          strokeDasharray={circumference}
          strokeDashoffset={circumference * (1 - clamped / 100)}
          style={{ transition: "stroke-dashoffset 300ms ease" }}
        />
      </svg>
      {label && <span className="text-[11px] text-muted">{label}</span>}
    </div>
  );
}
