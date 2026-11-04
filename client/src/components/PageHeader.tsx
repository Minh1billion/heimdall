import type { ReactNode } from "react";
import { NAV } from "../nav";
import { IconTile } from "./IconTile";
import { hasPageMark, PageMark } from "./PageMark";

interface PageHeaderProps {
  to: string;
  description?: string;
  actions?: ReactNode;
}

export function PageHeader({ to, description, actions }: PageHeaderProps) {
  const item = NAV.find((entry) => entry.to === to);
  if (!item) return null;

  return (
    <header className="page-header">
      {hasPageMark(item.to) ? <PageMark to={item.to} /> : <IconTile icon={item.icon} size="lg" />}
      <div className="page-header-text">
        <h1>{item.label}</h1>
        <p>{description ?? item.summary}</p>
      </div>
      {actions && <div className="page-header-actions">{actions}</div>}
    </header>
  );
}
