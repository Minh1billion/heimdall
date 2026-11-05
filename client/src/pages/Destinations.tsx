import { Activity, CircleCheck, Eye, EyeOff, FileText, LoaderCircle, Pencil, Plus, TriangleAlert, Users } from "lucide-react";
import { useEffect, useState } from "react";
import { api, type AdAccount, type Destination, type ReportTemplate } from "../api";
import { AccountPicker } from "../components/AccountPicker";
import { useAsync } from "../hooks";
import { CHANNEL_CHOICES, CHANNEL_FIELDS } from "../destinationMeta";
import { EmptyState } from "../components/EmptyState";
import { Field } from "../components/Field";
import { BrandIcon } from "../components/IconTile";
import { Modal } from "../components/Modal";
import { PageHeader } from "../components/PageHeader";
import { Result, type Outcome } from "../components/Result";
import { Select } from "../components/Select";
import { StatusBadge, type StatusInfo } from "../components/StatusBadge";
import { Switch } from "../components/Switch";
import { Tooltip } from "../components/Tooltip";
import { When } from "../components/When";

function labelOf(channel: string): string {
  return CHANNEL_CHOICES.find((entry) => entry.value === channel)?.label ?? channel;
}

function statusOf(destination: Destination): StatusInfo {
  if (!destination.enabled) {
    return { tone: "neutral", label: "Paused", tip: "Reports are not sent here until you turn it back on." };
  }
  if (destination.last_error) {
    return { tone: "fail", label: "Needs attention", icon: TriangleAlert, tip: destination.last_error };
  }
  if (destination.last_sent_at) {
    return { tone: "ok", label: "Delivered", icon: CircleCheck, tip: "The last message reached this destination." };
  }
  return { tone: "info", label: "Not used yet", tip: "Press Test to send a message." };
}

interface ModalProps {
  destination: Destination | null;
  templates: ReportTemplate[];
  accounts: AdAccount[];
  onClose: () => void;
  onChange: (items: Destination[]) => void;
}

function DestinationModal({ destination, templates, accounts, onClose, onChange }: ModalProps) {
  const [name, setName] = useState(destination?.name ?? "");
  const [channel, setChannel] = useState(destination?.channel ?? "zalo");
  const [values, setValues] = useState<Record<string, string>>(destination?.values ?? {});
  const [enabled, setEnabled] = useState(destination?.enabled ?? true);
  const [templateId, setTemplateId] = useState(destination?.template_id ?? 0);
  const [accountIds, setAccountIds] = useState<string[]>(destination?.account_ids ?? []);
  const [revealed, setRevealed] = useState<Record<string, boolean>>({});
  const [confirming, setConfirming] = useState(false);
  const [outcome, setOutcome] = useState<Outcome>({ status: "idle" });
  const busy = outcome.status === "running";
  const saving = outcome.status === "running" && outcome.label === "Saving";
  const fields = CHANNEL_FIELDS[channel] ?? [];
  const complete = fields.every((field) => {
    const typed = (values[field.key] ?? "").trim() !== "";
    return typed || (destination !== null && field.secret);
  });
  const canSave = name.trim() !== "" && complete;

  const save = async () => {
    setOutcome({ status: "running", label: "Saving" });
    const config: Record<string, string> = {};
    for (const field of fields) config[field.key] = (values[field.key] ?? "").trim();
    try {
      if (destination === null) {
        const created = await api.createDestination({
          name,
          channel,
          config,
          template_id: templateId || null,
          account_ids: accountIds,
        });
        onChange(created.destinations);
      } else {
        const updated = await api.updateDestination(destination.id, {
          name,
          config,
          enabled,
          template_id: templateId || null,
          account_ids: accountIds,
        });
        onChange(updated.destinations);
      }
    } catch (error) {
      setOutcome({ status: "error", message: error instanceof Error ? error.message : "Save failed" });
      return;
    }
    onClose();
  };

  const remove = async () => {
    if (destination === null) return;
    setOutcome({ status: "running", label: "Removing" });
    try {
      const result = await api.deleteDestination(destination.id);
      onChange(result.destinations);
      onClose();
    } catch (error) {
      setConfirming(false);
      setOutcome({ status: "error", message: error instanceof Error ? error.message : "Remove failed" });
    }
  };

  const confirmBox = (
    <div className="result">
      <p className="result-text">
        Remove “{destination?.name}”? Schedules that only send here are turned off.
      </p>
      <div className="confirm-actions">
        <button type="button" className="btn" disabled={busy} onClick={() => setConfirming(false)}>
          Keep destination
        </button>
        <button type="button" className="btn danger solid" disabled={busy} onClick={() => void remove()}>
          {busy && <LoaderCircle size={16} className="spin" />}
          Remove destination
        </button>
      </div>
    </div>
  );

  return (
    <Modal
      title={destination ? "Edit destination" : "Add destination"}
      description="One chat, group, webhook or mailing list that can receive reports."
      lead={<BrandIcon name={channel} size="md" />}
      busy={busy}
      status={confirming ? confirmBox : outcome.status === "error" ? <Result outcome={outcome} /> : null}
      onClose={onClose}
      onSubmit={() => {
        if (canSave) void save();
      }}
      footer={
        <>
          {destination && (
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
        <Field label="Name" hint="Who receives this, for example the partner or group name.">
          <input
            className="select"
            type="text"
            value={name}
            maxLength={60}
            placeholder="Partner A"
            disabled={busy}
            autoComplete="off"
            spellCheck={false}
            aria-label="Name"
            onChange={(event) => setName(event.target.value)}
          />
        </Field>

        {destination === null && (
          <Field label="Channel" hint="Where the report is delivered.">
            <Select
              ariaLabel="Channel"
              value={channel}
              onChange={(next) => {
                setChannel(next);
                setValues({});
              }}
              options={CHANNEL_CHOICES.map((entry) => ({
                value: entry.value,
                label: entry.label,
                icon: <BrandIcon name={entry.value} size="sm" />,
              }))}
            />
          </Field>
        )}

        {fields.map((field) => {
          const show = revealed[field.key] === true;
          const placeholder =
            destination && field.secret ? `${destination.secrets[field.key] ?? ""} - leave empty to keep` : field.placeholder;
          return (
            <Field key={`${channel}-${field.key}`} label={field.label} hint={field.hint}>
              {field.secret ? (
                <div className="secret-row">
                  <input
                    className="select"
                    type={show ? "text" : "password"}
                    value={values[field.key] ?? ""}
                    placeholder={placeholder}
                    disabled={busy}
                    autoComplete="new-password"
                    spellCheck={false}
                    aria-label={field.label}
                    onChange={(event) => setValues({ ...values, [field.key]: event.target.value })}
                  />
                  <Tooltip content={show ? "Hide what you typed" : "Show what you typed"} align="end">
                    <button
                      type="button"
                      className="icon-button"
                      aria-label={show ? "Hide value" : "Show value"}
                      onClick={() => setRevealed({ ...revealed, [field.key]: !show })}
                    >
                      {show ? <EyeOff size={16} /> : <Eye size={16} />}
                    </button>
                  </Tooltip>
                </div>
              ) : (
                <input
                  className="select"
                  type="text"
                  value={values[field.key] ?? ""}
                  placeholder={placeholder}
                  disabled={busy}
                  autoComplete="off"
                  spellCheck={false}
                  aria-label={field.label}
                  onChange={(event) => setValues({ ...values, [field.key]: event.target.value })}
                />
              )}
            </Field>
          );
        })}

        <Field
          label="Report template"
          hint="Which numbers, language and wording this destination receives. Manage templates on the Templates page."
        >
          <Select
            ariaLabel="Report template"
            value={templateId}
            onChange={setTemplateId}
            options={[
              { value: 0, label: "Default (full report)" },
              ...templates.map((entry) => ({ value: entry.id, label: entry.name })),
            ]}
          />
        </Field>

        <Field
          label="Ad accounts"
          hint="Which ad accounts this destination reports on. Tick several to get one message per account. Tick none to send the total of all accounts."
        >
          <AccountPicker accounts={accounts} selected={accountIds} onChange={setAccountIds} />
        </Field>

        {destination && (
          <div className="toggle-row">
            <div>
              <div className="toggle-title">Send reports here</div>
              <div className="muted small">Turn off to pause this destination without deleting it.</div>
            </div>
            <Switch checked={enabled} onChange={setEnabled} label="Send reports here" />
          </div>
        )}
      </div>
    </Modal>
  );
}

interface CardProps {
  destination: Destination;
  templateName: string;
  accountsLabel: string;
  onEdit: () => void;
  onChange: (items: Destination[]) => void;
}

function DestinationCard({ destination, templateName, accountsLabel, onEdit, onChange }: CardProps) {
  const [test, setTest] = useState<Outcome>({ status: "idle" });
  const testing = test.status === "running";

  const runTest = async () => {
    setTest({ status: "running", label: "Testing" });
    try {
      const result = await api.testDestination(destination.id);
      onChange(result.destinations);
      setTest({ status: "ok", message: result.message, model: "", latency: 0 });
    } catch (error) {
      setTest({ status: "error", message: error instanceof Error ? error.message : "Test failed" });
      try {
        onChange((await api.destinations()).destinations);
      } catch {
      }
    }
  };

  return (
    <li className={destination.enabled ? "item setting-card" : "item setting-card off"}>
      <div className="item-head">
        <BrandIcon name={destination.channel} size="md" />
        <div className="item-title">
          <h3 className="truncate" title={destination.name}>
            {destination.name}
          </h3>
          <p className="item-sentence mono truncate" title={destination.summary}>
            {labelOf(destination.channel)} · {destination.summary}
          </p>
        </div>
      </div>

      <div className="meta-chips">
        <span className="meta-chip">
          <FileText size={14} />
          <span className="truncate">{templateName}</span>
        </span>
        <span className="meta-chip">
          <BrandIcon name="meta" size="sm" />
          <span className="truncate">{accountsLabel}</span>
        </span>
      </div>

      <div className="setting-foot">
        <StatusBadge {...statusOf(destination)} />
        <span className="grow" />
        {destination.last_sent_at && (
          <span className="foot-item">
            <When value={destination.last_sent_at} align="end" />
          </span>
        )}
        <button type="button" className="btn" disabled={testing} onClick={() => void runTest()}>
          {testing ? <LoaderCircle size={15} className="spin" /> : <Activity size={15} />}
          Test
        </button>
        <button type="button" className="btn" onClick={onEdit}>
          <Pencil size={15} />
          Edit
        </button>
      </div>

      {(test.status === "ok" || test.status === "error") && <Result outcome={test} />}
    </li>
  );
}

export function Destinations() {
  const [items, setItems] = useState<Destination[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [open, setOpen] = useState<number | "new" | null>(null);

  useEffect(() => {
    api
      .destinations()
      .then((result) => setItems(result.destinations))
      .catch((failure: unknown) => setError(failure instanceof Error ? failure.message : "Request failed"));
  }, []);

  const templates = useAsync(() => api.templates(), []);
  const accounts = useAsync(() => api.adsAccounts(), []);
  const templateList = templates.data?.templates ?? [];
  const accountList = accounts.data?.accounts ?? [];
  const templateName = (id: number | null) =>
    id === null ? "Default report" : (templateList.find((entry) => entry.id === id)?.name ?? "Default report");
  const accountsLabel = (ids: string[]) =>
    ids.length === 0
      ? "All accounts combined"
      : ids.length === 1
        ? (accountList.find((entry) => entry.id === ids[0])?.name ?? ids[0])
        : `${ids.length} accounts, one message each`;
  const list = items ?? [];
  const editing = typeof open === "number" ? (list.find((item) => item.id === open) ?? null) : null;

  return (
    <div className="page">
      <PageHeader to="/destinations" />
      {error && <p className="notice error">{error}</p>}
      {items === null && !error && <p className="muted small">Loading destinations…</p>}

      {items !== null && (
        <section className="section">
          {list.length === 0 && (
            <EmptyState
              icon={Users}
              title="No destinations yet"
              text="Add the Zalo chat, Telegram group, Slack channel or inbox that should receive reports."
            />
          )}
          <ul className="card-grid">
            {list.map((destination) => (
              <DestinationCard
                key={destination.id}
                destination={destination}
                templateName={templateName(destination.template_id)}
                accountsLabel={accountsLabel(destination.account_ids)}
                onEdit={() => setOpen(destination.id)}
                onChange={setItems}
              />
            ))}
            <li className="add-card">
              <button type="button" className="add-card-button" onClick={() => setOpen("new")}>
                <Plus size={18} />
                {list.length === 0 ? "Add a destination" : "Add another destination"}
                <small>One per partner chat, group or inbox</small>
              </button>
            </li>
          </ul>
        </section>
      )}

      {open === "new" && (
        <DestinationModal
          destination={null}
          templates={templateList}
          accounts={accountList}
          onClose={() => setOpen(null)}
          onChange={setItems}
        />
      )}
      {editing && (
        <DestinationModal
          key={editing.id}
          destination={editing}
          templates={templateList}
          accounts={accountList}
          onClose={() => setOpen(null)}
          onChange={setItems}
        />
      )}
    </div>
  );
}
