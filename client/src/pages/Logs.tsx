import { ChevronDown, Coins, Layers, Rows3, Timer } from "lucide-react";
import { useState } from "react";
import { api } from "../api";
import { EmptyState } from "../components/EmptyState";
import { PageHeader } from "../components/PageHeader";
import { StatusBadge } from "../components/StatusBadge";
import { Tooltip } from "../components/Tooltip";
import { When } from "../components/When";
import { formatMs, formatNumber } from "../format";
import { useAsync } from "../hooks";
import { runStatus } from "../statusMeta";

function RunDetail({ id }: { id: string }) {
  const detail = useAsync(() => api.run(id), [id]);
  const data = detail.data;

  if (detail.error) return <p className="notice error">{detail.error}</p>;
  if (!data) return <p className="muted small">Loading…</p>;

  return (
    <div className="run-detail">
      {data.error && <p className="notice error">{data.error}</p>}
      {data.output && <p className="result-text">{data.output}</p>}
      <h4>Tool calls</h4>
      {data.tool_calls.length === 0 ? (
        <p className="muted small">The agent answered without calling any tool.</p>
      ) : (
        <ul className="trace">
          {data.tool_calls.map((call, index) => (
            <li key={index}>
              <div className="trace-head">
                <StatusBadge tone={call.ok ? "ok" : "fail"} label={call.ok ? "ok" : call.error_code ?? "error"} />
                <span className="mono truncate">{call.tool_name}</span>
                <span className="muted small">
                  step {call.step} · {formatMs(call.latency_ms)}
                </span>
              </div>
              <pre className="mono">{call.arguments}</pre>
              <pre className="mono muted">{call.result}</pre>
            </li>
          ))}
        </ul>
      )}
      <h4>Model requests</h4>
      <ul className="trace">
        {data.model_requests.map((request, index) => (
          <li key={index}>
            <div className="trace-head">
              <StatusBadge tone={request.ok ? "ok" : "fail"} label={request.ok ? "ok" : request.error_code ?? "error"} />
              <span className="mono truncate">
                {request.provider} · {request.model}
              </span>
              <span className="muted small">
                {formatNumber(request.input_tokens)} in · {formatNumber(request.output_tokens)} out · {formatMs(request.latency_ms)}
              </span>
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}

export function Logs() {
  const runs = useAsync(() => api.runs(), []);
  const [open, setOpen] = useState<string | null>(null);

  return (
    <div className="page">
      <PageHeader to="/logs" />
      {runs.error && <p className="notice error">{runs.error}</p>}
      {runs.data && runs.data.length === 0 && (
        <EmptyState icon={Rows3} title="No runs yet" text="Agent runs appear here after you ask a question in Chat." />
      )}
      {runs.data && runs.data.length > 0 && (
        <ul className="card list">
          {runs.data.map((run) => {
            const expanded = open === run.id;
            return (
              <li key={run.id} className="row">
                <button
                  type="button"
                  className="run-row"
                  aria-expanded={expanded}
                  onClick={() => setOpen(expanded ? null : run.id)}
                >
                  <span className="run-title">
                    <ChevronDown size={16} className={expanded ? "chev open" : "chev"} />
                    <span className="truncate">{run.input}</span>
                  </span>
                  <span className="run-meta">
                    <StatusBadge {...runStatus(run.status)} focusable={false} />
                    <Tooltip content="Agent steps taken to answer">
                      <span className="foot-item">
                        <Layers size={14} />
                        {run.steps}
                      </span>
                    </Tooltip>
                    <Tooltip content="Total tokens used (input plus output)">
                      <span className="foot-item">
                        <Coins size={14} />
                        {formatNumber(run.total_tokens)}
                      </span>
                    </Tooltip>
                    <Tooltip content="Total run time">
                      <span className="foot-item">
                        <Timer size={14} />
                        {formatMs(run.duration_ms)}
                      </span>
                    </Tooltip>
                    <span className="muted small run-when">
                      <When value={run.created_at} align="end" focusable={false} />
                    </span>
                  </span>
                </button>
                {expanded && <RunDetail id={run.id} />}
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
