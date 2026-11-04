import { Check, Clock, History, List, LoaderCircle, Megaphone, Quote, ShieldCheck, TriangleAlert, X } from "lucide-react";
import { useState } from "react";
import { api, type ApprovalStatus } from "../api";
import { EmptyState } from "../components/EmptyState";
import { BrandIcon, IconTile } from "../components/IconTile";
import { PageHeader } from "../components/PageHeader";
import { Segmented } from "../components/Segmented";
import { StatusBadge } from "../components/StatusBadge";
import { Tooltip } from "../components/Tooltip";
import { When } from "../components/When";
import { useAsync } from "../hooks";
import { approvalStatus, toolLabel } from "../statusMeta";

type Filter = "pending" | "all";

export function Approvals() {
  const [filter, setFilter] = useState<Filter>("pending");
  const [version, setVersion] = useState(0);
  const [busy, setBusy] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const status: ApprovalStatus | null = filter === "pending" ? "pending" : null;
  const approvals = useAsync(() => api.approvals(status), [filter, version]);

  const decide = async (id: number, approve: boolean) => {
    setBusy(id);
    setError(null);
    try {
      await (approve ? api.approve(id) : api.reject(id));
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : "Request failed");
    } finally {
      setBusy(null);
      setVersion((value) => value + 1);
    }
  };

  const items = approvals.data?.items ?? [];
  const pending = approvals.data?.pending ?? 0;

  return (
    <div className="page">
      <PageHeader to="/approvals" />
      {approvals.data && !approvals.data.writes_enabled && (
        <p className="notice error">Set a Meta access token with ads_management access in Settings to apply approved changes.</p>
      )}

      <div className="toolbar tight">
        <Segmented
          ariaLabel="Filter requests"
          value={filter}
          onChange={setFilter}
          options={[
            { value: "pending", label: approvals.data ? `Pending (${pending})` : "Pending", icon: Clock, tip: "Requests waiting for your decision" },
            { value: "all", label: "All", icon: List, tip: "Every request, including decided ones" },
          ]}
        />
      </div>
      {error && <p className="notice error">{error}</p>}

      {approvals.data && items.length === 0 && (
        <EmptyState
          icon={ShieldCheck}
          title={filter === "pending" ? "Nothing waiting for approval" : "No requests yet"}
          text="When the agent proposes pausing a campaign or changing a budget, the request shows up here."
        />
      )}

      {items.length > 0 && (
        <ul className="card-grid wide">
          {items.map((item) => {
            const tool = toolLabel(item.tool_name);
            const decided = item.status !== "pending";
            return (
              <li key={item.id} className="item">
                <div className="item-head">
                  <Tooltip content={tool.tip} align="start">
                    <span tabIndex={0}>
                      <IconTile icon={tool.icon} tone={tool.tone} />
                    </span>
                  </Tooltip>
                  <div className="item-title">
                    <h3>{item.summary}</h3>
                    <p className="item-sentence">{tool.label}</p>
                  </div>
                  <StatusBadge {...approvalStatus(item.status)} />
                </div>

                <div className="meta-chips">
                  <Tooltip content={`Campaign ID ${item.campaign_id}`} align="start">
                    <span className="meta-chip" tabIndex={0}>
                      <Megaphone size={14} />
                      <span className="truncate">{item.campaign_name || item.campaign_id}</span>
                    </span>
                  </Tooltip>
                  <Tooltip content="Ad account this campaign belongs to">
                    <span className="meta-chip" tabIndex={0}>
                      <BrandIcon name="meta" size="sm" />
                      <span className="truncate mono">{item.account_id}</span>
                    </span>
                  </Tooltip>
                </div>

                {item.reason && (
                  <div className="quote-box">
                    <Quote size={14} />
                    <p>{item.reason}</p>
                  </div>
                )}

                {item.result && (
                  <div className={item.status === "failed" ? "item-error" : "result-box"} role={item.status === "failed" ? "alert" : undefined}>
                    {item.status === "failed" && <TriangleAlert size={15} />}
                    <span>{item.result}</span>
                  </div>
                )}

                <div className="item-foot">
                  <span className="foot-item">
                    <History size={14} />
                    Proposed <When value={item.created_at} align="start" />
                  </span>
                  <span className="foot-item">
                    <Clock size={14} />
                    {decided ? "Decided " : "Expires "}
                    <When value={decided ? item.decided_at : item.expires_at} align="end" />
                  </span>
                </div>

                {!decided && (
                  <div className="decision">
                    <Tooltip content="Reject: nothing will change" align="start">
                      <button type="button" className="btn" disabled={busy === item.id} onClick={() => decide(item.id, false)}>
                        <X size={16} /> Reject
                      </button>
                    </Tooltip>
                    <Tooltip content="Approve and apply this change on Meta" align="end">
                      <button type="button" className="btn primary" disabled={busy === item.id} onClick={() => decide(item.id, true)}>
                        {busy === item.id ? <LoaderCircle size={16} className="spin" /> : <Check size={16} />} Approve
                      </button>
                    </Tooltip>
                  </div>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
