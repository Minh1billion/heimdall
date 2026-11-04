import { Info } from "lucide-react";
import type { ReactNode } from "react";
import { Tooltip } from "./Tooltip";

interface FieldProps {
  label: string;
  hint?: ReactNode;
  children: ReactNode;
}

export function Field({ label, hint, children }: FieldProps) {
  return (
    <div className="field">
      <span className="field-label">
        {label}
        {hint && (
          <Tooltip content={hint} align="start">
            <button type="button" className="info-button" aria-label={`About ${label}`}>
              <Info size={13} />
            </button>
          </Tooltip>
        )}
      </span>
      {children}
    </div>
  );
}
