import type { LucideIcon } from "lucide-react";
import { type KeyboardEvent, useRef } from "react";
import { Tooltip } from "./Tooltip";

export interface SegmentedOption<T extends string> {
  value: T;
  label: string;
  icon?: LucideIcon;
  tip?: string;
}

interface SegmentedProps<T extends string> {
  value: T;
  options: SegmentedOption<T>[];
  onChange: (value: T) => void;
  ariaLabel: string;
}

export function Segmented<T extends string>({ value, options, onChange, ariaLabel }: SegmentedProps<T>) {
  const root = useRef<HTMLDivElement>(null);

  const onKeyDown = (event: KeyboardEvent) => {
    const index = options.findIndex((option) => option.value === value);
    let next = index;
    if (event.key === "ArrowRight" || event.key === "ArrowDown") next = (index + 1) % options.length;
    else if (event.key === "ArrowLeft" || event.key === "ArrowUp") next = (index - 1 + options.length) % options.length;
    else return;
    event.preventDefault();
    onChange(options[next].value);
    root.current?.querySelectorAll<HTMLButtonElement>("button")[next]?.focus();
  };

  return (
    <div className="segmented" role="radiogroup" aria-label={ariaLabel} ref={root} onKeyDown={onKeyDown}>
      {options.map((option) => {
        const selected = option.value === value;
        const Icon = option.icon;
        const button = (
          <button
            key={option.value}
            type="button"
            role="radio"
            aria-checked={selected}
            tabIndex={selected ? 0 : -1}
            className={selected ? "segment on" : "segment"}
            onClick={() => onChange(option.value)}
          >
            {Icon && <Icon size={15} />}
            <span>{option.label}</span>
          </button>
        );
        return option.tip ? (
          <Tooltip key={option.value} content={option.tip}>
            {button}
          </Tooltip>
        ) : (
          button
        );
      })}
    </div>
  );
}
