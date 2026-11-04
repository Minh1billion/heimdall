import {
  Bot,
  FlaskConical,
  Hash,
  Mail,
  Megaphone,
  MessageCircle,
  Plug,
  Send,
  Server,
  Sparkles,
  type LucideIcon,
} from "lucide-react";

type Tone = "neutral" | "green" | "red" | "blue" | "amber";
type Size = "sm" | "md" | "lg";

interface IconTileProps {
  icon: LucideIcon;
  size?: Size;
  tone?: Tone;
}

const PIXELS: Record<Size, number> = { sm: 16, md: 18, lg: 22 };

export function IconTile({ icon: Icon, size = "md", tone = "neutral" }: IconTileProps) {
  return (
    <span className={`tile ${size} ${tone}`} aria-hidden="true">
      <Icon size={PIXELS[size]} />
    </span>
  );
}

const logos = import.meta.glob("../assets/brands/*.svg", { eager: true, query: "?url", import: "default" }) as Record<string, string>;
const logoUrls: Record<string, string> = Object.fromEntries(
  Object.entries(logos).map(([path, url]) => [path.split("/").pop()!.replace(".svg", ""), url]),
);

const FALLBACK: Record<string, LucideIcon> = {
  openai: Sparkles,
  anthropic: Bot,
  openai_compat: Server,
  fake: FlaskConical,
  telegram: Send,
  slack: Hash,
  zalo: MessageCircle,
  email: Mail,
  meta: Megaphone,
};

interface BrandIconProps {
  name: string;
  size?: Size;
}

export function BrandIcon({ name, size = "md" }: BrandIconProps) {
  const url = logoUrls[name];
  if (url) {
    return (
      <span className={`tile ${size} logo`} aria-hidden="true">
        <img src={url} alt="" />
      </span>
    );
  }
  return <IconTile icon={FALLBACK[name] ?? Plug} size={size} />;
}
