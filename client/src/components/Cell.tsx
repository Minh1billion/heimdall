import { Info, type LucideIcon } from "lucide-react";
import type { ReactNode } from "react";
import { Tooltip } from "./Tooltip";

interface CellProps {
  title: string;
  icon?: LucideIcon;
  hint?: ReactNode;
  aside?: ReactNode;
  children: ReactNode;
}

export function Cell({ title, icon: Icon, hint, aside, children }: CellProps) {
  return (
    <section className="cell">
      <header className="cell-head">
        <h3>
          {Icon && <Icon size={16} />}
          {title}
          {hint && (
            <Tooltip content={hint} side="bottom" align="start">
              <button type="button" className="info-button" aria-label={`About ${title}`}>
                <Info size={13} />
              </button>
            </Tooltip>
          )}
        </h3>
        {aside}
      </header>
      {children}
    </section>
  );
}
