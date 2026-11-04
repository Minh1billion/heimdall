import { type ReactNode, useId } from "react";

interface TooltipProps {
  content: ReactNode;
  children: ReactNode;
  side?: "top" | "bottom";
  align?: "center" | "start" | "end";
}

export function Tooltip({ content, children, side = "top", align = "center" }: TooltipProps) {
  const id = useId();
  return (
    <span className="tip" aria-describedby={id}>
      {children}
      <span id={id} role="tooltip" className={`tip-bubble ${side} ${align}`}>
        {content}
      </span>
    </span>
  );
}
