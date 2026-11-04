import type { Range } from "../api";
import { Segmented } from "./Segmented";

const OPTIONS: { value: Range; label: string; tip: string }[] = [
  { value: "24h", label: "24h", tip: "Last 24 hours" },
  { value: "7d", label: "7d", tip: "Last 7 days" },
  { value: "14d", label: "14d", tip: "Last 14 days" },
  { value: "30d", label: "30d", tip: "Last 30 days" },
];

interface RangeToggleProps {
  value: Range;
  onChange: (value: Range) => void;
}

export function RangeToggle({ value, onChange }: RangeToggleProps) {
  return (
    <div className="range-toggle">
      <Segmented ariaLabel="Time range" value={value} onChange={onChange} options={OPTIONS} />
    </div>
  );
}
