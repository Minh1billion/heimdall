import type { ReactNode } from "react";

interface RowLeadProps {
  icon: ReactNode;
  title: ReactNode;
  sub?: ReactNode;
  mono?: boolean;
}

export function RowLead({ icon, title, sub, mono = false }: RowLeadProps) {
  return (
    <div className="row-lead">
      {icon}
      <div className="row-lead-text">
        <div className="row-title">{title}</div>
        {sub && <div className={mono ? "row-sub mono" : "row-sub"}>{sub}</div>}
      </div>
    </div>
  );
}
