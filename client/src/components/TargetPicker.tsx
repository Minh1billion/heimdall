import { BrandIcon } from "./IconTile";

export interface Target {
  value: string;
  label: string;
  channel: string;
  hint: string;
}

interface TargetPickerProps {
  targets: Target[];
  selected: string[];
  onChange: (next: string[]) => void;
}

export function TargetPicker({ targets, selected, onChange }: TargetPickerProps) {
  const toggle = (value: string) =>
    onChange(selected.includes(value) ? selected.filter((item) => item !== value) : [...selected, value]);
  const all = targets.length > 0 && targets.every((target) => selected.includes(target.value));

  return (
    <div className="targets">
      <div className="targets-head">
        <span className="muted small">
          {targets.filter((target) => selected.includes(target.value)).length} of {targets.length} selected
        </span>
        <button type="button" className="btn" onClick={() => onChange(all ? [] : targets.map((target) => target.value))}>
          {all ? "Clear" : "Select all"}
        </button>
      </div>
      <ul className="targets-list">
        {targets.map((target) => {
          const on = selected.includes(target.value);
          return (
            <li key={target.value}>
              <label className={on ? "target on" : "target"}>
                <input type="checkbox" checked={on} onChange={() => toggle(target.value)} />
                <BrandIcon name={target.channel} size="sm" />
                <span className="target-text">
                  <strong className="truncate">{target.label}</strong>
                  <small className="truncate">{target.hint}</small>
                </span>
              </label>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
