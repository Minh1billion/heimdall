import {
  Activity,
  Bell,
  CircleCheck,
  Eye,
  EyeOff,
  Globe,
  ListOrdered,
  Lock,
  LockOpen,
  LoaderCircle,
  Megaphone,
  Pencil,
  RotateCcw,
  Settings2,
  ShieldCheck,
  Sparkles,
  type LucideIcon,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import {
  api,
  getAdminToken,
  setAdminToken,
  type MetaConnection,
  type SettingField,
  type SettingGroup,
  type SettingSource,
  type SettingsPayload,
} from "../api";
import { BrandIcon, IconTile } from "../components/IconTile";
import { Field } from "../components/Field";
import { MetaConnections } from "../components/MetaConnections";
import { Modal } from "../components/Modal";
import { Result, type Outcome } from "../components/Result";
import { PageHeader } from "../components/PageHeader";
import { SectionHead } from "../components/SectionHead";
import { StatusBadge } from "../components/StatusBadge";
import { Tooltip } from "../components/Tooltip";

const BRANDED = new Set(["openai", "anthropic", "openai_compat", "telegram", "slack", "zalo", "email", "meta"]);
const PLAIN_ICONS: Record<string, LucideIcon> = { llm: ListOrdered, write_limits: ShieldCheck, general: Globe };

const SOURCE_LABEL: Record<SettingSource, string> = { app: "Set in app", default: "Default" };
const SOURCE_TIP: Record<SettingSource, string> = {
  app: "Saved in Heimdall. Use the reset button to go back to the built-in default.",
  default: "Not set yet, so Heimdall uses its built-in default.",
};

function GroupIcon({ group }: { group: SettingGroup }) {
  if (BRANDED.has(group.key)) return <BrandIcon name={group.key} size="md" />;
  return <IconTile icon={PLAIN_ICONS[group.key] ?? Sparkles} size="md" />;
}

function summaryOf(group: SettingGroup): string | null {
  const model = group.fields.find((field) => field.key.endsWith("_model"));
  if (model) return model.value || "No model set";
  const order = group.fields.find((field) => field.kind === "order");
  if (order) return order.value;
  return null;
}

interface FieldInputProps {
  field: SettingField;
  value: string;
  revealed: boolean;
  reverting: boolean;
  disabled: boolean;
  onChange: (value: string) => void;
  onReveal: () => void;
  onToggleReset: () => void;
}

function FieldInput({ field, value, revealed, reverting, disabled, onChange, onReveal, onToggleReset }: FieldInputProps) {
  const placeholder = field.secret
    ? field.is_set
      ? `${field.preview ?? "••••"} - leave empty to keep`
      : field.placeholder || "Not set"
    : field.placeholder;
  const input = (
    <input
      className="select"
      type={field.secret && !revealed ? "password" : "text"}
      value={reverting ? "" : value}
      placeholder={reverting ? "Will reset to default" : placeholder}
      disabled={disabled || reverting}
      autoComplete={field.secret ? "new-password" : "off"}
      spellCheck={false}
      aria-label={field.label}
      onChange={(event) => onChange(event.target.value)}
    />
  );
  return (
    <Field label={field.label} hint={field.hint}>
      {field.secret ? (
        <div className="secret-row">
          {input}
          <Tooltip content={revealed ? "Hide what you typed" : "Show what you typed"} align="end">
            <button type="button" className="icon-button" aria-label={revealed ? "Hide value" : "Show value"} onClick={onReveal}>
              {revealed ? <EyeOff size={16} /> : <Eye size={16} />}
            </button>
          </Tooltip>
        </div>
      ) : (
        input
      )}
      <div className="field-meta">
        <Tooltip content={SOURCE_TIP[field.source]} align="start">
          <span className={`field-source ${field.source}`} tabIndex={0}>
            {SOURCE_LABEL[field.source]}
          </span>
        </Tooltip>
        {field.source === "app" && (
          <button type="button" className="field-reset" disabled={disabled} onClick={onToggleReset}>
            <RotateCcw size={12} />
            {reverting ? "Keep saved value" : "Reset to default"}
          </button>
        )}
      </div>
    </Field>
  );
}

interface GroupCardProps {
  group: SettingGroup;
  editable: boolean;
  onSaved: (payload: SettingsPayload) => void;
}

function draftOf(group: SettingGroup): Record<string, string> {
  return Object.fromEntries(group.fields.map((field) => [field.key, field.secret ? "" : field.value ?? ""]));
}

function GroupCard({ group, editable, onSaved }: GroupCardProps) {
  const [open, setOpen] = useState(false);
  const [draft, setDraft] = useState<Record<string, string>>({});
  const [revealed, setRevealed] = useState<Record<string, boolean>>({});
  const [reverting, setReverting] = useState<Set<string>>(new Set());
  const [outcome, setOutcome] = useState<Outcome>({ status: "idle" });
  const [test, setTest] = useState<Outcome>({ status: "idle" });
  const busy = outcome.status === "running";
  const testing = test.status === "running";
  const summary = summaryOf(group);

  const begin = () => {
    setDraft(draftOf(group));
    setRevealed({});
    setReverting(new Set());
    setOutcome({ status: "idle" });
    setTest({ status: "idle" });
    setOpen(true);
  };

  const toggleReset = (key: string) => {
    const next = new Set(reverting);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    setReverting(next);
  };

  const submit = async () => {
    const values: Record<string, string> = {};
    for (const field of group.fields) {
      if (reverting.has(field.key)) continue;
      const typed = (draft[field.key] ?? "").trim();
      if (typed === "") continue;
      if (!field.secret && typed === (field.value ?? "")) continue;
      values[field.key] = typed;
    }
    const clear = [...reverting];
    setOutcome({ status: "running", label: "Saving" });
    let next: SettingsPayload;
    try {
      next = await api.saveSettings(group.key, values, clear);
    } catch (error) {
      setOutcome({ status: "error", message: error instanceof Error ? error.message : "Save failed" });
      return;
    }
    onSaved(next);
    setTest({ status: "idle" });
    setOutcome({ status: "idle" });
    setOpen(false);
  };

  const runTest = async () => {
    setTest({ status: "running", label: "Testing" });
    try {
      const result = await api.testSettings(group.key);
      setTest({ status: "ok", message: result.message, model: result.model, latency: result.latency_ms });
    } catch (error) {
      setTest({ status: "error", message: error instanceof Error ? error.message : "Test failed" });
    }
  };

  const footer = (
    <>
      <span className="grow" />
      <button type="submit" className="btn primary" disabled={busy}>
        {busy && <LoaderCircle size={16} className="spin" />}
        {busy ? "Saving" : "Save"}
      </button>
    </>
  );

  return (
    <li className="item setting-card">
      <div className="item-head">
        <GroupIcon group={group} />
        <div className="item-title">
          <h3 className="truncate">{group.label}</h3>
          <p className={summary ? "item-sentence mono truncate" : "item-sentence truncate"}>{summary ?? group.description}</p>
        </div>
      </div>

      <div className="setting-foot">
        {group.configured === true && (
          <StatusBadge tone="ok" label="Configured" icon={CircleCheck} tip="Heimdall has what it needs to use this." />
        )}
        {group.configured === false && (
          <StatusBadge tone="neutral" label="Not configured" tip="Open Edit and fill in the credentials." />
        )}
        <span className="grow" />
        {group.priority !== null && (
          <Tooltip content="Position in the provider order. Lower numbers are tried first." align="end">
            <span className="foot-item" tabIndex={0}>
              <ListOrdered size={14} />
              {group.priority}
            </span>
          </Tooltip>
        )}
        {group.testable && (
          <Tooltip
            content={
              !editable
                ? "Unlock editing first"
                : group.configured === false
                  ? "Fill in the credentials first"
                  : "Check the saved settings"
            }
            align="end"
          >
            <span>
              <button
                type="button"
                className="btn"
                disabled={!editable || group.configured === false || testing}
                onClick={() => void runTest()}
              >
                {testing ? <LoaderCircle size={15} className="spin" /> : <Activity size={15} />}
                Test
              </button>
            </span>
          </Tooltip>
        )}
        <Tooltip
          content={editable ? (group.configured === false ? "Fill in the credentials" : "Edit these settings") : "Unlock editing first"}
          align="end"
        >
          <span>
            <button type="button" className="btn" disabled={!editable} onClick={begin}>
              {group.configured === false ? <Settings2 size={15} /> : <Pencil size={15} />}
              {group.configured === false ? "Configuration" : "Edit"}
            </button>
          </span>
        </Tooltip>
      </div>

      {(test.status === "ok" || test.status === "error") && <Result outcome={test} />}

      {open && (
        <Modal
          title={group.label}
          description={group.description}
          lead={<GroupIcon group={group} />}
          busy={busy}
          status={outcome.status === "error" ? <Result outcome={outcome} /> : null}
          footer={footer}
          onClose={() => setOpen(false)}
          onSubmit={() => void submit()}
        >
          <div className="fields">
            {group.fields.map((field) => (
              <FieldInput
                key={field.key}
                field={field}
                value={draft[field.key] ?? ""}
                revealed={revealed[field.key] ?? false}
                reverting={reverting.has(field.key)}
                disabled={busy}
                onChange={(value) => setDraft({ ...draft, [field.key]: value })}
                onReveal={() => setRevealed({ ...revealed, [field.key]: !revealed[field.key] })}
                onToggleReset={() => toggleReset(field.key)}
              />
            ))}
          </div>
        </Modal>
      )}
    </li>
  );
}

interface AccessProps {
  payload: SettingsPayload;
  onChange: () => void;
}

function Access({ payload, onChange }: AccessProps) {
  const { access } = payload;
  const [token, setToken] = useState("");
  const rejected = access.requires_token && !access.editable && getAdminToken() !== "";

  if (!access.requires_token) {
    return access.editable ? null : (
      <p className="notice error">
        <Lock size={14} className="inline-icon" /> {access.reason}
      </p>
    );
  }

  if (access.editable) {
    return (
      <div className="access">
        <LockOpen size={16} />
        <span>Editing is unlocked for this browser tab.</span>
        <span className="grow" />
        <button
          type="button"
          className="btn"
          onClick={() => {
            setAdminToken("");
            onChange();
          }}
        >
          Lock
        </button>
      </div>
    );
  }

  return (
    <form
      className="access"
      onSubmit={(event) => {
        event.preventDefault();
        setAdminToken(token.trim());
        setToken("");
        onChange();
      }}
    >
      <Lock size={16} />
      <span>{rejected ? "That admin token was not accepted." : "Enter the admin token to edit settings."}</span>
      <span className="grow" />
      <input
        className="select"
        type="password"
        value={token}
        placeholder="Admin token"
        autoComplete="off"
        aria-label="Admin token"
        onChange={(event) => setToken(event.target.value)}
      />
      <button type="submit" className="btn primary" disabled={token.trim() === ""}>
        Unlock
      </button>
    </form>
  );
}

interface SectionProps {
  payload: SettingsPayload;
  keys: string[];
  onSaved: (payload: SettingsPayload) => void;
}

function Cards({ payload, keys, onSaved }: SectionProps) {
  return (
    <ul className="card-grid">
      {keys.map((key) => {
        const group = payload.groups.find((item) => item.key === key);
        return group ? <GroupCard key={key} group={group} editable={payload.access.editable} onSaved={onSaved} /> : null;
      })}
    </ul>
  );
}

export function Settings() {
  const [payload, setPayload] = useState<SettingsPayload | null>(null);
  const [connections, setConnections] = useState<MetaConnection[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    Promise.all([api.settings(), api.metaConnections()])
      .then(([data, meta]) => {
        setPayload(data);
        setConnections(meta.connections);
        setError(null);
      })
      .catch((failure: Error) => setError(failure.message));
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  return (
    <div className="page settings-page">
      <PageHeader
        to="/settings"
        description="Everything here is saved in Heimdall and applies immediately, with no restart. Keys and tokens are stored encrypted and are never shown again after saving."
      />
      {error && <p className="notice error">{error}</p>}
      {!payload && !error && <p className="muted small">Loading settings…</p>}

      {payload && connections && (
        <>
          <Access payload={payload} onChange={load} />

          <section className="settings-section">
            <SectionHead
              icon={Sparkles}
              title="Language models"
              text="Heimdall tries providers in priority order and falls back to the next one when a request fails."
            />
            <Cards payload={payload} keys={["openai", "anthropic", "openai_compat", "llm"]} onSaved={setPayload} />
          </section>

          <section className="settings-section">
            <SectionHead icon={Bell} title="Notification channels" text="Reports and alerts are delivered to the channels below." />
            <Cards payload={payload} keys={["telegram", "slack", "zalo", "email"]} onSaved={setPayload} />
          </section>

          <section className="settings-section">
            <SectionHead
              icon={Megaphone}
              title="Meta ads"
              text="Add one token for each business or client. Heimdall syncs every ad account a token can see."
            />
            <MetaConnections connections={connections} editable={payload.access.editable} onChange={setConnections} />
          </section>

          <section className="settings-section">
            <SectionHead
              icon={ShieldCheck}
              title="Limits and general"
              text="How often Meta syncs, safeguards for agent changes and the clock used by schedules."
            />
            <Cards payload={payload} keys={["meta", "write_limits", "general"]} onSaved={setPayload} />
          </section>
        </>
      )}
    </div>
  );
}
