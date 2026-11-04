import { Check, ChevronDown } from "lucide-react";
import { type KeyboardEvent, type ReactNode, useEffect, useId, useLayoutEffect, useRef, useState } from "react";

export interface SelectOption<T extends string | number> {
  value: T;
  label: string;
  icon?: ReactNode;
  hint?: string;
  description?: string;
}

interface SelectProps<T extends string | number> {
  value: T;
  options: SelectOption<T>[];
  onChange: (value: T) => void;
  ariaLabel?: string;
}

export function Select<T extends string | number>({ value, options, onChange, ariaLabel }: SelectProps<T>) {
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const [up, setUp] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLUListElement>(null);
  const id = useId();
  const selectedIndex = Math.max(0, options.findIndex((option) => option.value === value));
  const selected = options[selectedIndex];

  const show = () => {
    setActive(selectedIndex);
    setOpen(true);
  };

  const choose = (index: number) => {
    const option = options[index];
    if (!option) return;
    onChange(option.value);
    setOpen(false);
    trigger.current?.focus();
  };

  useEffect(() => {
    if (!open) return;
    const handler = (event: MouseEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  useLayoutEffect(() => {
    if (!open || !root.current || !menu.current) return;
    const rect = root.current.getBoundingClientRect();
    const height = menu.current.offsetHeight + 12;
    setUp(window.innerHeight - rect.bottom < height && rect.top > height);
  }, [open]);

  useEffect(() => {
    if (open) menu.current?.querySelector('[data-active="true"]')?.scrollIntoView({ block: "nearest" });
  }, [open, active]);

  const onKeyDown = (event: KeyboardEvent) => {
    if (!open) {
      if (["ArrowDown", "ArrowUp", "Enter", " "].includes(event.key)) {
        event.preventDefault();
        show();
      }
      return;
    }
    switch (event.key) {
      case "ArrowDown":
        setActive((current) => (current + 1) % options.length);
        break;
      case "ArrowUp":
        setActive((current) => (current - 1 + options.length) % options.length);
        break;
      case "Home":
        setActive(0);
        break;
      case "End":
        setActive(options.length - 1);
        break;
      case "Enter":
      case " ":
        choose(active);
        break;
      case "Escape":
        setOpen(false);
        break;
      case "Tab":
        setOpen(false);
        return;
      default:
        return;
    }
    event.preventDefault();
  };

  return (
    <div className="select-root" ref={root} onKeyDown={onKeyDown}>
      <button
        ref={trigger}
        type="button"
        className={open ? "select-trigger open" : "select-trigger"}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        aria-label={ariaLabel}
        onClick={() => (open ? setOpen(false) : show())}
      >
        {selected?.icon}
        <span className="select-value truncate">{selected?.label}</span>
        <ChevronDown size={16} className="select-chevron" />
      </button>
      {open && (
        <ul
          ref={menu}
          id={id}
          role="listbox"
          tabIndex={-1}
          className={up ? "select-menu up" : "select-menu"}
          aria-activedescendant={`${id}-${active}`}
        >
          {options.map((option, index) => (
            <li
              key={String(option.value)}
              id={`${id}-${index}`}
              role="option"
              aria-selected={option.value === value}
              data-active={index === active}
              className={index === active ? "select-option active" : "select-option"}
              onMouseEnter={() => setActive(index)}
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => choose(index)}
            >
              {option.icon}
              {option.description ? (
                <span className="select-text">
                  <span className="select-label">{option.label}</span>
                  <span className="select-desc">{option.description}</span>
                </span>
              ) : (
                <span className="select-label">{option.label}</span>
              )}
              {option.hint && <span className="select-hint">{option.hint}</span>}
              {option.value === value && <Check size={16} className="select-check" />}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
