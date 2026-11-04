import { ChevronLeft, ChevronRight, X } from "lucide-react";
import { useState } from "react";
import { Link } from "react-router-dom";

interface Slide {
  title: string;
  text: string;
  action: { label: string; to: string } | null;
}

const SLIDES: Slide[] = [
  {
    title: "Connect a language model",
    text: "Add an API key for OpenAI, Anthropic or a local model so Heimdall can analyze your ads.",
    action: { label: "Open settings", to: "/settings" },
  },
  {
    title: "Connect your ad accounts",
    text: "Link one or more Facebook ad accounts to pull spend, results and trends automatically.",
    action: { label: "Open ad accounts", to: "/ad-accounts" },
  },
  {
    title: "Ask your first question",
    text: "Chat with your ad data and review every agent run in the logs.",
    action: { label: "Open chat", to: "/chat" },
  },
];

interface BannerProps {
  onDismiss: () => void;
}

export function Banner({ onDismiss }: BannerProps) {
  const [index, setIndex] = useState(0);
  const slide = SLIDES[index];
  const step = (delta: number) => setIndex((index + delta + SLIDES.length) % SLIDES.length);

  return (
    <>
      <div className="carousel-nav">
        <span>
          {index + 1} of {SLIDES.length}
        </span>
        <button type="button" className="icon-button" aria-label="Previous" onClick={() => step(-1)}>
          <ChevronLeft size={18} />
        </button>
        <button type="button" className="icon-button" aria-label="Next" onClick={() => step(1)}>
          <ChevronRight size={18} />
        </button>
      </div>
      <div className="banner">
        <button type="button" className="banner-close" aria-label="Dismiss" onClick={onDismiss}>
          <X size={16} />
        </button>
        <h2>{slide.title}</h2>
        <p>{slide.text}</p>
        {slide.action ? (
          <Link className="btn light" to={slide.action.to}>
            {slide.action.label}
          </Link>
        ) : (
          <button type="button" className="btn light" disabled>
            Coming soon
          </button>
        )}
      </div>
    </>
  );
}
