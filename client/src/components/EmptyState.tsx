import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

interface EmptyStateProps {
  icon?: LucideIcon;
  mark?: ReactNode;
  title: string;
  text: string;
  action?: ReactNode;
}

export function EmptyState({ icon: Icon, mark, title, text, action }: EmptyStateProps) {
  return (
    <div className="empty">
      {mark ?? (
        Icon && (
          <span className="empty-icon">
            <Icon size={22} />
          </span>
        )
      )}
      <h3>{title}</h3>
      <p>{text}</p>
      {action}
    </div>
  );
}
