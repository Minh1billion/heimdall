import { CircleCheck, TriangleAlert } from "lucide-react";
import { formatNumber } from "../format";

export type Outcome =
  | { status: "idle" }
  | { status: "running"; label: string }
  | { status: "ok"; message: string; model: string; latency: number }
  | { status: "error"; message: string };

export function Result({ outcome }: { outcome: Outcome }) {
  if (outcome.status === "ok") {
    return (
      <div className="result ok">
        <p className="result-text">
          <CircleCheck size={15} className="inline-icon" /> {outcome.message}
        </p>
        {(outcome.model || outcome.latency > 0) && (
          <dl className="result-meta">
            {outcome.model && (
              <div>
                <dt>Model</dt>
                <dd className="mono">{outcome.model}</dd>
              </div>
            )}
            {outcome.latency > 0 && (
              <div>
                <dt>Latency</dt>
                <dd>{formatNumber(outcome.latency)} ms</dd>
              </div>
            )}
          </dl>
        )}
      </div>
    );
  }
  if (outcome.status === "error") {
    return (
      <div className="result error">
        <p className="result-text">
          <TriangleAlert size={15} className="inline-icon" /> {outcome.message}
        </p>
      </div>
    );
  }
  return null;
}
