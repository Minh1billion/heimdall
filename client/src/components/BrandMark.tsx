interface BrandMarkProps {
  className?: string;
}

export function BrandMark({ className }: BrandMarkProps) {
  return (
    <svg className={className} viewBox="0 0 32 32" aria-hidden="true">
      <rect width="32" height="32" rx="8" fill="#0d0d0d" />
      <rect x="8" y="7" width="3.4" height="18" rx="1.7" fill="#fff" />
      <rect x="20.6" y="7" width="3.4" height="18" rx="1.7" fill="#fff" />
      <path d="M10 20Q16 9.5 22 20" fill="none" stroke="#fff" strokeWidth="3" strokeLinecap="round" />
    </svg>
  );
}
