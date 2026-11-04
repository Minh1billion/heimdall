import type { Channel } from "../api";
import { BrandIcon } from "./IconTile";
import { Select } from "./Select";

interface ChannelSelectProps {
  channels: Channel[];
  value: string;
  onChange: (value: string) => void;
}

export function ChannelSelect({ channels, value, onChange }: ChannelSelectProps) {
  return (
    <Select
      ariaLabel="Channel"
      value={value}
      onChange={onChange}
      options={channels.map((channel) => ({
        value: channel.name,
        label: channel.label,
        icon: <BrandIcon name={channel.name} size="sm" />,
        hint: channel.configured ? undefined : "Not configured",
      }))}
    />
  );
}
