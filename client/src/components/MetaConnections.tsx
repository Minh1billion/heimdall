import { Activity, Building2, CircleCheck, Eye, EyeOff, LoaderCircle, Pencil, Plus, TriangleAlert } from "lucide-react";
import { useState } from "react";
import { api, type MetaConnection } from "../api";
import { Field } from "./Field";
import { BrandIcon } from "./IconTile";
import { Modal } from "./Modal";
import { Result, type Outcome } from "./Result";
import { StatusBadge, type StatusInfo } from "./StatusBadge";
import { Switch } from "./Switch";
import { Tooltip } from "./Tooltip";

const NAME_HINT = "A label to tell tokens apart, for example the client or business it belongs to.";
const TOKEN_HINT =
  "A token with ads_read access. A system user token works best. Add ads_management to allow approved changes. It is stored encrypted and never shown again.";

function statusOf(connection: MetaConnection): StatusInfo {
  if (!connection.enabled) {
    return { tone: "neutral", label: "Paused", tip: "Not used for syncing or changes until you turn it back on." };
  }
  if (connection.last_error) return { tone: "fail", label: "Needs attention", icon: TriangleAlert, tip: connection.last_error };
  if (connection.last_synced_at) return { tone: "ok", label: "Connected", icon: CircleCheck, tip: "The last sync with this token worked." };
  return { tone: "info", label: "Not synced yet", tip: "Press Sync now on Ad accounts, or wait for the next automatic sync." };
}

interface ModalProps {
  connection: MetaConnection | null;
  onClose: () => void;
  onChange: (connections: MetaConnection[]) => void;
}

function ConnectionModal({ connection, onClose, onChange }: ModalProps) {
  const [name, setName] = useState(connection?.name ?? "");
  const [token, setToken] = useState("");
  const [enabled, setEnabled] = useState(connection?.enabled ?? true);
  const [revealed, setRevealed] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [outcome, setOutcome] = useState<Outcome>({ status: "idle" });
  const busy = outcome.status === "running";
  const saving = outcome.status === "running" && outcome.label === "Saving";
  const canSave = name.trim() !== "" && (connection !== null || token.trim() !== "");

  const save = async () => {
    setOutcome({ status: "running", label: "Saving" });
    try {
      if (connection === null) {
        const created = await api.createMetaConnection(name, token);
        onChange(created.connections);
      } else {
        const patch: { name?: string; token?: string; enabled?: boolean } = {};
        if (name.trim() !== connection.name) patch.name = name;
        if (token.trim() !== "") patch.token = token;
        if (enabled !== connection.enabled) patch.enabled = enabled;
        if (Object.keys(patch).length > 0) {
          const updated = await api.updateMetaConnection(connection.id, patch);
          onChange(updated.connections);
        }
      }
    } catch (error) {
      setOutcome({ status: "error", message: error instanceof Error ? error.message : "Save failed" });
      return;
    }
    onClose();
  };

  const remove = async () => {
    if (connection === null) return;
    setOutcome({ status: "running", label: "Removing" });
    try {
      const result = await api.deleteMetaConnection(connection.id);
      onChange(result.connections);
      onClose();
    } catch (error) {
      setConfirming(false);
      setOutcome({ status: "error", message: error instanceof Error ? error.message : "Remove failed" });
    }
  };

  const placeholder = connection ? `${connection.preview} - leave empty to keep` : "EAAB...";

  const confirmBox = (
    <div className="result">
      <p className="result-text">
        Remove “{connection?.name}”? Its {connection?.accounts ?? 0} ad account{connection?.accounts === 1 ? "" : "s"} and their data stay in Heimdall, but
        they cannot sync or be changed until another token covers them.
      </p>
      <div className="confirm-actions">
        <button type="button" className="btn" disabled={busy} onClick={() => setConfirming(false)}>
          Keep token
        </button>
        <button type="button" className="btn danger solid" disabled={busy} onClick={() => void remove()}>
          {busy && <LoaderCircle size={16} className="spin" />}
          Remove token
        </button>
      </div>
    </div>
  );

  return (
    <Modal
      title={connection ? "Edit Meta token" : "Add Meta token"}
      description="Each token syncs every ad account it can see."
      lead={<BrandIcon name="meta" size="md" />}
      busy={busy}
      status={confirming ? confirmBox : outcome.status === "error" ? <Result outcome={outcome} /> : null}
      onClose={onClose}
      onSubmit={() => {
        if (canSave) void save();
      }}
      footer={
        <>
          {connection && (
            <button type="button" className="btn danger" disabled={busy || confirming} onClick={() => setConfirming(true)}>
              Remove
            </button>
          )}
          <span className="grow" />
          <button type="submit" className="btn primary" disabled={busy || confirming || !canSave}>
            {saving && <LoaderCircle size={16} className="spin" />}
            {saving ? "Saving" : "Save"}
          </button>
        </>
      }
    >
      <div className="fields">
        <Field label="Name" hint={NAME_HINT}>
          <input
            className="select"
            type="text"
            value={name}
            maxLength={60}
            placeholder="Client A"
            disabled={busy}
            autoComplete="off"
            spellCheck={false}
            aria-label="Name"
            onChange={(event) => setName(event.target.value)}
          />
        </Field>

        <Field label="Access token" hint={TOKEN_HINT}>
          <div className="secret-row">
            <input
              className="select"
              type={revealed ? "text" : "password"}
              value={token}
              placeholder={placeholder}
              disabled={busy}
              autoComplete="new-password"
              spellCheck={false}
              aria-label="Access token"
              onChange={(event) => setToken(event.target.value)}
            />
            <Tooltip content={revealed ? "Hide what you typed" : "Show what you typed"} align="end">
              <button
                type="button"
                className="icon-button"
                aria-label={revealed ? "Hide value" : "Show value"}
                onClick={() => setRevealed(!revealed)}
              >
                {revealed ? <EyeOff size={16} /> : <Eye size={16} />}
              </button>
            </Tooltip>
          </div>
        </Field>

        {connection && (
          <div className="toggle-row">
            <div>
              <div className="toggle-title">Use this token</div>
              <div className="muted small">Turn off to pause syncing and approved changes for its ad accounts.</div>
            </div>
            <Switch checked={enabled} onChange={setEnabled} label="Use this token" />
          </div>
        )}
      </div>
    </Modal>
  );
}

interface CardProps {
  connection: MetaConnection;
  editable: boolean;
  onEdit: () => void;
  onChange: (connections: MetaConnection[]) => void;
}

function ConnectionCard({ connection, editable, onEdit, onChange }: CardProps) {
  const [test, setTest] = useState<Outcome>({ status: "idle" });
  const testing = test.status === "running";

  const runTest = async () => {
    setTest({ status: "running", label: "Testing" });
    try {
      const result = await api.testMetaConnection(connection.id);
      onChange(result.connections);
      setTest({ status: "ok", message: result.message, model: "", latency: 0 });
    } catch (error) {
      setTest({ status: "error", message: error instanceof Error ? error.message : "Test failed" });
      try {
        onChange((await api.metaConnections()).connections);
      } catch {
      }
    }
  };

  return (
    <li className={connection.enabled ? "item setting-card" : "item setting-card off"}>
      <div className="item-head">
        <BrandIcon name="meta" size="md" />
        <div className="item-title">
          <h3 className="truncate" title={connection.name}>
            {connection.name}
          </h3>
          <p className="item-sentence mono truncate">{connection.preview}</p>
        </div>
      </div>

      <div className="setting-foot">
        <StatusBadge {...statusOf(connection)} />
        <span className="grow" />
        <Tooltip content="Ad accounts synced with this token" align="end">
          <span className="foot-item" tabIndex={0}>
            <Building2 size={14} />
            {connection.accounts}
          </span>
        </Tooltip>
        <Tooltip content={editable ? "Check that Meta accepts this token" : "Unlock editing first"} align="end">
          <span>
            <button type="button" className="btn" disabled={!editable || testing} onClick={() => void runTest()}>
              {testing ? <LoaderCircle size={15} className="spin" /> : <Activity size={15} />}
              Test
            </button>
          </span>
        </Tooltip>
        <Tooltip content={editable ? "Edit this token" : "Unlock editing first"} align="end">
          <span>
            <button type="button" className="btn" disabled={!editable} onClick={onEdit}>
              <Pencil size={15} />
              Edit
            </button>
          </span>
        </Tooltip>
      </div>

      {(test.status === "ok" || test.status === "error") && <Result outcome={test} />}
    </li>
  );
}

interface MetaConnectionsProps {
  connections: MetaConnection[];
  editable: boolean;
  onChange: (connections: MetaConnection[]) => void;
}

export function MetaConnections({ connections, editable, onChange }: MetaConnectionsProps) {
  const [open, setOpen] = useState<number | "new" | null>(null);
  const editing = typeof open === "number" ? (connections.find((item) => item.id === open) ?? null) : null;

  return (
    <>
      <ul className="card-grid">
        {connections.map((connection) => (
          <ConnectionCard
            key={connection.id}
            connection={connection}
            editable={editable}
            onEdit={() => setOpen(connection.id)}
            onChange={onChange}
          />
        ))}
        <li className="add-card">
          <button type="button" className="add-card-button" disabled={!editable} onClick={() => setOpen("new")}>
            <Plus size={18} />
            {connections.length === 0 ? "Add a Meta token" : "Add another token"}
            <small>One per business or client</small>
          </button>
        </li>
      </ul>

      {open === "new" && <ConnectionModal connection={null} onClose={() => setOpen(null)} onChange={onChange} />}
      {editing && <ConnectionModal key={editing.id} connection={editing} onClose={() => setOpen(null)} onChange={onChange} />}
    </>
  );
}
