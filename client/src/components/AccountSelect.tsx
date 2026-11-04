import type { AdAccount } from "../api";
import { BrandIcon } from "./IconTile";
import { Select } from "./Select";

interface AccountSelectProps {
  accounts: AdAccount[];
  value: string | null;
  onChange: (value: string | null) => void;
}

export function AccountSelect({ accounts, value, onChange }: AccountSelectProps) {
  return (
    <Select
      ariaLabel="Ad account"
      value={value ?? ""}
      onChange={(next) => onChange(next || null)}
      options={[
        { value: "", label: "All accounts" },
        ...accounts.map((account) => ({
          value: account.id,
          label: account.name,
          icon: <BrandIcon name="meta" size="sm" />,
          hint: account.id,
        })),
      ]}
    />
  );
}
