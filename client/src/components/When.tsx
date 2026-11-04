import { formatDateTime, formatRelative } from "../format";
import { Tooltip } from "./Tooltip";

interface WhenProps {
  value: string | null;
  align?: "center" | "start" | "end";
  focusable?: boolean;
}

export function When({ value, align = "center", focusable = true }: WhenProps) {
  if (!value) return <span>Never</span>;
  return (
    <Tooltip content={formatDateTime(value)} align={align}>
      <span tabIndex={focusable ? 0 : undefined}>{formatRelative(value)}</span>
    </Tooltip>
  );
}
