import { Link } from "react-router-dom";
import { EmptyState } from "../components/EmptyState";
import { NAV } from "../nav";

interface ComingSoonProps {
  path: string;
}

export function ComingSoon({ path }: ComingSoonProps) {
  const item = NAV.find((entry) => entry.to === path);
  if (!item) return null;

  return (
    <div className="page">
      <h1>{item.label}</h1>
      <EmptyState
        icon={item.icon}
        title={`${item.label} is not available yet`}
        text={`${item.summary ?? ""} It arrives in checkpoint ${item.checkpoint}.`.trim()}
        action={
          <Link className="btn" to="/">
            Back to Home
          </Link>
        }
      />
    </div>
  );
}
