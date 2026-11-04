import type { LucideIcon } from "lucide-react";
import { Tooltip } from "./Tooltip";

export type StatusTone = "ok" | "warn" | "fail" | "neutral" | "info";

export interface StatusInfo {
  tone: StatusTone;
  label: string;
  icon?: LucideIcon;
  tip?: string;
}

interface StatusBadgeProps extends StatusInfo {
  focusable?: boolean;
}

export function StatusBadge({ tone, label, icon: Icon, tip, focusable = true }: StatusBadgeProps) {
  const badge = (
    <span className={`status ${tone}`} tabIndex={tip && focusable ? 0 : undefined}>
      {Icon && <Icon size={13} />}
      {label}
    </span>
  );
  return tip ? (
    <Tooltip content={tip} align="end">
      {badge}
    </Tooltip>
  ) : (
    badge
  );
}
