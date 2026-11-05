import type { AdAccount } from "../api";
import { BrandIcon } from "./IconTile";

interface AccountPickerProps {
  accounts: AdAccount[];
  selected: string[];
  onChange: (next: string[]) => void;
  allLabel?: string;
}

export function AccountPicker({ accounts, selected, onChange, allLabel = "All accounts combined" }: AccountPickerProps) {
  const toggle = (id: string) => onChange(selected.includes(id) ? selected.filter((item) => item !== id) : [...selected, id]);

  return (
    <div className="targets">
      <div className="targets-head">
        <span className="muted small">{selected.length === 0 ? allLabel : `${selected.length} of ${accounts.length} selected`}</span>
        <button type="button" className="btn" disabled={selected.length === 0} onClick={() => onChange([])}>
          Use all
        </button>
      </div>
      <ul className="targets-list">
        {accounts.map((account) => {
          const on = selected.includes(account.id);
          return (
            <li key={account.id}>
              <label className={on ? "target on" : "target"}>
                <input type="checkbox" checked={on} onChange={() => toggle(account.id)} />
                <BrandIcon name="meta" size="sm" />
                <span className="target-text">
                  <strong className="truncate">{account.name}</strong>
                  <small className="truncate">{account.id}</small>
                </span>
              </label>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
