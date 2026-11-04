import type { LucideIcon } from "lucide-react";
import { IconTile } from "./IconTile";

interface SectionHeadProps {
  icon: LucideIcon;
  title: string;
  text?: string;
}

export function SectionHead({ icon, title, text }: SectionHeadProps) {
  return (
    <div className="section-head">
      <IconTile icon={icon} size="sm" />
      <div>
        <h2 className="section-heading">{title}</h2>
        {text && <p className="section-text">{text}</p>}
      </div>
    </div>
  );
}
