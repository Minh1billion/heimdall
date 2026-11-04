import type { ReactNode } from "react";

const INK = "#0d0d0d";

const stroke = { fill: "none", stroke: "#fff", strokeWidth: 3, strokeLinecap: "round", strokeLinejoin: "round" } as const;

function Bar({ x, y, w, h }: { x: number; y: number; w: number; h: number }) {
  return <rect x={x} y={y} width={w} height={h} rx={1.7} fill="#fff" />;
}

const GLYPHS: Record<string, ReactNode> = {
  "/": <path d="M8.5 24.5V15.5Q16 2 23.5 15.5V24.5Z" {...stroke} />,
  "/chat": <path d="M8.5 11Q8.5 8.5 11 8.5H21Q23.5 8.5 23.5 11V17.5Q23.5 20 21 20H15.5L11 24V20Q8.5 20 8.5 17.5Z" {...stroke} />,
  "/ad-accounts": (
    <>
      <path d="M8.5 24.5V9.5Q8.5 8 10 8H16.5Q18 8 18 9.5V24.5Z" {...stroke} />
      <path d="M18 15H22Q23.5 15 23.5 16.5V24.5H18" {...stroke} />
      <path d="M13.25 13.5H13.26M13.25 19.5H13.26" {...stroke} />
    </>
  ),
  "/insights": (
    <>
      <Bar x={7.6} y={16} w={3.4} h={9} />
      <Bar x={14.3} y={7} w={3.4} h={18} />
      <Bar x={21} y={11.5} w={3.4} h={13.5} />
    </>
  ),
  "/reports": (
    <>
      <path d="M10 22.5V9.5Q10 8 11.5 8H17L22 13V22.5Q22 24 20.5 24H11.5Q10 24 10 22.5Z" {...stroke} />
      <path d="M13.5 19H18.5" {...stroke} />
    </>
  ),
  "/alerts": (
    <>
      <path d="M9.5 19.5Q12.5 18 12.5 13A3.5 3.5 0 0 1 19.5 13Q19.5 18 22.5 19.5Z" {...stroke} />
      <path d="M14.2 24Q16 25.8 17.8 24" {...stroke} />
    </>
  ),
  "/approvals": (
    <>
      <path d="M16 7.5L23 10V15.5Q23 21 16 24.5Q9 21 9 15.5V10Z" {...stroke} />
      <path d="M12.9 16L15.2 18.3L19.2 13.9" {...stroke} strokeWidth={2.6} />
    </>
  ),
  "/logs": (
    <>
      <Bar x={8} y={7.6} w={16} h={3.4} />
      <Bar x={8} y={14.3} w={16} h={3.4} />
      <Bar x={8} y={21} w={10} h={3.4} />
    </>
  ),
  "/settings": (
    <>
      <path d="M7.5 11H24.5M7.5 21H24.5" {...stroke} />
      <circle cx={19.5} cy={11} r={3} fill={INK} stroke="#fff" strokeWidth={3} />
      <circle cx={12.5} cy={21} r={3} fill={INK} stroke="#fff" strokeWidth={3} />
    </>
  ),
};

interface PageMarkProps {
  to: string;
}

export function PageMark({ to }: PageMarkProps) {
  return (
    <svg className="page-mark" viewBox="0 0 32 32" aria-hidden="true">
      <rect width="32" height="32" rx="8" fill={INK} />
      {GLYPHS[to]}
    </svg>
  );
}

export function hasPageMark(to: string): boolean {
  return to in GLYPHS;
}
