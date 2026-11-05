import { LayoutTemplate, LoaderCircle, Pencil, Plus } from "lucide-react";
import { useState } from "react";
import { api, type ReportTemplate, type TemplateInput } from "../api";
import { EmptyState } from "../components/EmptyState";
import { Field } from "../components/Field";
import { Modal } from "../components/Modal";
import { PageHeader } from "../components/PageHeader";
import { Result, type Outcome } from "../components/Result";
import { Segmented } from "../components/Segmented";
import { Select } from "../components/Select";
import { Switch } from "../components/Switch";
import { useAsync } from "../hooks";

const METRIC_LABELS: Record<string, string> = {
  spend: "Spend",
  impressions: "Impressions",
  clicks: "Clicks",
  conversions: "Conversions",
  ctr: "CTR",
  cpc: "CPC",
  cpm: "CPM",
  cpa: "CPA",
  roas: "ROAS",
};

const CAMPAIGN_CHOICES = [0, 1, 3, 5, 10].map((value) => ({
  value,
  label: value === 0 ? "Hide campaigns" : `Top ${value} campaigns`,
}));

const BLANK: TemplateInput = {
  name: "",
  metrics: ["spend", "impressions", "clicks", "conversions", "cpa", "roas"],
  campaigns: 3,
  anomalies: true,
  insight: false,
  language: "en",
  header: "",
  footer: "",
};

interface ModalProps {
  template: ReportTemplate | null;
  metrics: string[];
  onClose: () => void;
  onChange: (items: ReportTemplate[]) => void;
}

function TemplateModal({ template, metrics, onClose, onChange }: ModalProps) {
  const [form, setForm] = useState<TemplateInput>(
    template
      ? {
          name: template.name,
          metrics: template.metrics,
          campaigns: template.campaigns,
          anomalies: template.anomalies,
          insight: template.insight,
          language: template.language,
          header: template.header,
          footer: template.footer,
        }
      : BLANK,
  );
  const [confirming, setConfirming] = useState(false);
  const [outcome, setOutcome] = useState<Outcome>({ status: "idle" });
  const busy = outcome.status === "running";
  const canSave = form.name.trim() !== "" && form.metrics.length > 0;

  const toggle = (key: string) =>
    setForm({
      ...form,
      metrics: form.metrics.includes(key) ? form.metrics.filter((item) => item !== key) : [...form.metrics, key],
    });

  const save = async () => {
    setOutcome({ status: "running", label: "Saving" });
    try {
      const result = template ? await api.updateTemplate(template.id, form) : await api.createTemplate(form);
      onChange(result.templates);
      onClose();
    } catch (error) {
      setOutcome({ status: "error", message: error instanceof Error ? error.message : "Save failed" });
    }
  };

  const remove = async () => {
    if (!template) return;
    setOutcome({ status: "running", label: "Removing" });
    try {
      const result = await api.deleteTemplate(template.id);
      onChange(result.templates);
      onClose();
    } catch (error) {
      setConfirming(false);
      setOutcome({ status: "error", message: error instanceof Error ? error.message : "Remove failed" });
    }
  };

  const confirmBox = (
    <div className="result">
      <p className="result-text">
        Remove “{template?.name}”? Destinations that use it go back to the default report.
      </p>
      <div className="confirm-actions">
        <button type="button" className="btn" disabled={busy} onClick={() => setConfirming(false)}>
          Keep template
        </button>
        <button type="button" className="btn danger solid" disabled={busy} onClick={() => void remove()}>
          {busy && <LoaderCircle size={16} className="spin" />}
          Remove template
        </button>
      </div>
    </div>
  );

  return (
    <Modal
      title={template ? "Edit template" : "Add template"}
      description="Decides what a destination receives: which numbers, which language and what wording around them."
      busy={busy}
      status={confirming ? confirmBox : outcome.status === "error" ? <Result outcome={outcome} /> : null}
      onClose={onClose}
      onSubmit={() => {
        if (canSave) void save();
      }}
      footer={
        <>
          {template && (
            <button type="button" className="btn danger" disabled={busy || confirming} onClick={() => setConfirming(true)}>
              Remove
            </button>
          )}
          <span className="grow" />
          <button type="submit" className="btn primary" disabled={busy || confirming || !canSave}>
            {busy && <LoaderCircle size={16} className="spin" />}
            {busy ? "Saving" : "Save"}
          </button>
        </>
      }
    >
      <div className="fields one">
        <Field label="Name" hint="For example Partner summary or Internal full.">
          <input
            className="select"
            type="text"
            value={form.name}
            maxLength={60}
            placeholder="Partner summary"
            disabled={busy}
            autoComplete="off"
            spellCheck={false}
            aria-label="Name"
            onChange={(event) => setForm({ ...form, name: event.target.value })}
          />
        </Field>

        <Field label="Language" hint="Labels and the AI insight are written in this language.">
          <Segmented
            ariaLabel="Language"
            value={form.language}
            onChange={(language) => setForm({ ...form, language })}
            options={[
              { value: "en", label: "English" },
              { value: "vi", label: "Tiếng Việt" },
            ]}
          />
        </Field>

        <Field label="Numbers to show" hint="Only the ticked numbers appear in the message.">
          <div className="metric-grid">
            {metrics.map((key) => {
              const on = form.metrics.includes(key);
              return (
                <label key={key} className={on ? "target on" : "target"}>
                  <input type="checkbox" checked={on} disabled={busy} onChange={() => toggle(key)} />
                  <span className="target-text">
                    <strong className="truncate">{METRIC_LABELS[key] ?? key}</strong>
                  </span>
                </label>
              );
            })}
          </div>
        </Field>

        <Field label="Campaigns" hint="How many top campaigns by spend are listed.">
          <Select
            ariaLabel="Campaigns"
            value={form.campaigns}
            onChange={(campaigns) => setForm({ ...form, campaigns })}
            options={CAMPAIGN_CHOICES}
          />
        </Field>

        <div className="toggle-row">
          <div>
            <div className="toggle-title">Unusual days</div>
            <div className="muted small">List days where spend or CPA moved far from normal.</div>
          </div>
          <Switch checked={form.anomalies} onChange={(anomalies) => setForm({ ...form, anomalies })} label="Unusual days" />
        </div>

        <div className="toggle-row">
          <div>
            <div className="toggle-title">AI insight</div>
            <div className="muted small">Add a short written takeaway when the report is sent. Off by default.</div>
          </div>
          <Switch checked={form.insight} onChange={(insight) => setForm({ ...form, insight })} label="AI insight" />
        </div>

        <Field label="Opening line" hint="Optional. You can use {account} and {period}.">
          <textarea
            className="select area"
            value={form.header}
            maxLength={400}
            placeholder="Hello, here is your ad report for {period}."
            disabled={busy}
            aria-label="Opening line"
            onChange={(event) => setForm({ ...form, header: event.target.value })}
          />
        </Field>

        <Field label="Closing line" hint="Optional. You can use {account} and {period}.">
          <textarea
            className="select area"
            value={form.footer}
            maxLength={400}
            placeholder="Contact us if you have any questions."
            disabled={busy}
            aria-label="Closing line"
            onChange={(event) => setForm({ ...form, footer: event.target.value })}
          />
        </Field>
      </div>
    </Modal>
  );
}

export function Templates() {
  const loaded = useAsync(() => api.templates(), []);
  const [items, setItems] = useState<ReportTemplate[] | null>(null);
  const [open, setOpen] = useState<number | "new" | null>(null);

  const list = items ?? loaded.data?.templates ?? [];
  const metrics = loaded.data?.metrics ?? Object.keys(METRIC_LABELS);
  const editing = typeof open === "number" ? (list.find((item) => item.id === open) ?? null) : null;

  return (
    <div className="page">
      <PageHeader to="/templates" />
      {loaded.error && <p className="notice error">{loaded.error}</p>}
      {loaded.loading && items === null && <p className="muted small">Loading templates…</p>}

      {!loaded.loading && !loaded.error && (
        <section className="section">
          {list.length === 0 && (
            <EmptyState
              icon={LayoutTemplate}
              title="No templates yet"
              text="Without a template, destinations receive the default report. Add one to control what each partner sees."
              action={
                <button type="button" className="btn primary" onClick={() => setOpen("new")}>
                  <Plus size={16} />
                  Add a template
                </button>
              }
            />
          )}
          {list.length > 0 && (
          <ul className="card-grid">
            {list.map((template) => (
              <li key={template.id} className="item setting-card">
                <div className="item-head">
                  <div className="item-title">
                    <h3 className="truncate" title={template.name}>
                      {template.name}
                    </h3>
                    <p className="item-sentence truncate">
                      {template.language === "vi" ? "Tiếng Việt" : "English"} · {template.metrics.length} numbers ·{" "}
                      {template.campaigns === 0 ? "no campaigns" : `top ${template.campaigns} campaigns`}
                    </p>
                  </div>
                </div>
                <div className="meta-chips">
                  {template.metrics.map((key) => (
                    <span key={key} className="meta-chip">
                      {METRIC_LABELS[key] ?? key}
                    </span>
                  ))}
                </div>
                <div className="setting-foot">
                  <span className="muted small">
                    {template.used_by === 0 ? "Not used yet" : `Used by ${template.used_by} destination${template.used_by === 1 ? "" : "s"}`}
                  </span>
                  <span className="grow" />
                  <button type="button" className="btn" onClick={() => setOpen(template.id)}>
                    <Pencil size={15} />
                    Edit
                  </button>
                </div>
              </li>
            ))}
            <li className="add-card">
              <button type="button" className="add-card-button" onClick={() => setOpen("new")}>
                <Plus size={18} />
                Add another template
                <small>One per kind of partner or audience</small>
              </button>
            </li>
          </ul>
          )}
        </section>
      )}

      {open === "new" && <TemplateModal template={null} metrics={metrics} onClose={() => setOpen(null)} onChange={setItems} />}
      {editing && <TemplateModal key={editing.id} template={editing} metrics={metrics} onClose={() => setOpen(null)} onChange={setItems} />}
    </div>
  );
}
