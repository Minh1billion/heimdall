import {
  CalendarClock,
  CalendarDays,
  CalendarRange,
  Check,
  Clock,
  Copy,
  FilePlus2,
  FileText,
  History,
  LoaderCircle,
  Plus,
  Repeat,
  Send,
  SlidersHorizontal,
  Trash2,
  TriangleAlert,
  Users,
} from "lucide-react";
import { type FormEvent, useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { api, type BroadcastItem, type Destination, type EmailContext, type Report, type Schedule } from "../api";
import { AccountPicker } from "../components/AccountPicker";
import { AccountSelect } from "../components/AccountSelect";
import { ChannelPreview } from "../components/ChannelPreview";
import { EmptyState } from "../components/EmptyState";
import { Field } from "../components/Field";
import { BrandIcon, IconTile } from "../components/IconTile";
import { PageHeader } from "../components/PageHeader";
import { Result } from "../components/Result";
import { Segmented } from "../components/Segmented";
import { SectionHead } from "../components/SectionHead";
import { Select } from "../components/Select";
import { Switch } from "../components/Switch";
import { TargetPicker, type Target } from "../components/TargetPicker";
import { Tooltip } from "../components/Tooltip";
import { When } from "../components/When";
import { titleCase } from "../statusMeta";
import { useAsync } from "../hooks";

const WEEKDAYS = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

const TEMPLATES = {
  daily: { label: "Daily", icon: CalendarDays, covers: "yesterday", tip: "Results for yesterday." },
  weekly: { label: "Weekly", icon: CalendarRange, covers: "the last 7 days", tip: "Results for the last 7 days." },
} as const;

type Template = keyof typeof TEMPLATES;

type ScopeMode = "own" | "custom";

function when(item: Schedule): string {
  return item.weekday !== null ? `Every ${WEEKDAYS[item.weekday] ?? "week"} at ${item.time}` : `Every day at ${item.time}`;
}

export function Reports() {
  const accounts = useAsync(() => api.adsAccounts(), []);
  const channels = useAsync(() => api.channels(), []);
  const destinations = useAsync(() => api.destinations(), []);
  const templates = useAsync(() => api.templates(), []);
  const [version, setVersion] = useState(0);
  const schedules = useAsync(() => api.schedules(), [version]);

  const [template, setTemplate] = useState<Template>("daily");
  const [accountId, setAccountId] = useState<string | null>(null);
  const [scopeMode, setScopeMode] = useState<ScopeMode>("own");
  const [customAccounts, setCustomAccounts] = useState<string[]>([]);
  const [customTemplate, setCustomTemplate] = useState(0);
  const [selected, setSelected] = useState<string[]>([]);
  const [results, setResults] = useState<BroadcastItem[] | null>(null);
  const [report, setReport] = useState<Report | null>(null);
  const [sending, setSending] = useState(false);
  const [previewing, setPreviewing] = useState(false);
  const [message, setMessage] = useState<{ kind: "ok" | "error"; text: string } | null>(null);
  const [copied, setCopied] = useState(false);

  const [name, setName] = useState("");
  const [time, setTime] = useState("08:00");
  const [weekday, setWeekday] = useState(0);
  const [adding, setAdding] = useState(false);

  const customMode = scopeMode === "custom";
  const params = {
    template,
    account_id: customMode ? (customAccounts.length === 1 ? customAccounts[0] : null) : accountId,
  };
  const templateList = templates.data?.templates ?? [];
  const channelList = channels.data ?? [];
  const accountList = accounts.data?.accounts ?? [];
  const destinationList = destinations.data?.destinations ?? [];
  const targets: Target[] = [
    ...destinationList
      .filter((entry) => entry.enabled)
      .map((entry) => ({ value: `d:${entry.id}`, label: entry.name, channel: entry.channel, hint: entry.summary })),
    ...channelList
      .filter((entry) => entry.configured)
      .map((entry) => ({ value: `c:${entry.name}`, label: `${entry.label} (default)`, channel: entry.name, hint: "From Settings" })),
  ];
  const chosen = targets.filter((entry) => selected.includes(entry.value));
  const focus = chosen[0] ?? targets[0] ?? null;
  const previewChannel = focus?.channel ?? "telegram";
  const focusDestination = focus?.value.startsWith("d:")
    ? destinationList.find((entry) => entry.id === Number(focus.value.slice(2)))
    : undefined;
  const previewAccount = customMode ? (customAccounts[0] ?? null) : (focusDestination?.account_ids[0] ?? accountId);
  const previewTemplateId = customMode
    ? customTemplate || (focusDestination?.template_id ?? null)
    : (focusDestination?.template_id ?? null);
  const previewEmail: EmailContext | null =
    report?.email && focusDestination?.channel === "email"
      ? { from: report.email.from, to: focusDestination.recipients }
      : (report?.email ?? null);
  const isDestination = (entry: Target) => entry.value.startsWith("d:");
  const reason = (failure: unknown) => (failure instanceof Error ? failure.message : "Request failed");
  const accountName = (id: string | null) => (id ? (accountList.find((entry) => entry.id === id)?.name ?? id) : "All accounts");
  const channelName = (id: string) => channelList.find((entry) => entry.name === id)?.label ?? id;
  const fail = (failure: unknown) =>
    setMessage({ kind: "error", text: failure instanceof Error ? failure.message : "Request failed" });

  useEffect(() => {
    let active = true;
    setPreviewing(true);
    api
      .previewReport({ template, account_id: previewAccount, template_id: previewTemplateId })
      .then((result) => {
        if (active) setReport(result);
      })
      .catch((failure: unknown) => {
        if (active) fail(failure);
      })
      .finally(() => {
        if (active) setPreviewing(false);
      });
    return () => {
      active = false;
    };
  }, [template, previewAccount, previewTemplateId]);

  const send = async () => {
    if (chosen.length === 0) return;
    setSending(true);
    setMessage(null);
    setResults(null);
    const outcomes: BroadcastItem[] = [];
    const destinationTargets = chosen.filter(isDestination);
    try {
      if (destinationTargets.length > 0) {
        const sent = await api.broadcastReport(
          params,
          destinationTargets.map((entry) => Number(entry.value.slice(2))),
          customMode ? { account_ids: customAccounts, template_id: customTemplate || null } : undefined,
        );
        outcomes.push(...sent.results);
      }
    } catch (failure) {
      for (const entry of destinationTargets) {
        outcomes.push({
          destination_id: Number(entry.value.slice(2)),
          name: entry.label,
          channel: entry.channel,
          ok: false,
          skipped: false,
          error: reason(failure),
        });
      }
    }
    for (const entry of chosen.filter((item) => !isDestination(item))) {
      try {
        await api.sendReport(params, { channel: entry.value.slice(2) });
        outcomes.push({ destination_id: 0, name: entry.label, channel: entry.channel, ok: true, skipped: false, error: null });
      } catch (failure) {
        outcomes.push({ destination_id: 0, name: entry.label, channel: entry.channel, ok: false, skipped: false, error: reason(failure) });
      }
    }
    setResults(outcomes);
    setSending(false);
  };

  const create = async (event: FormEvent) => {
    event.preventDefault();
    if (chosen.length === 0) return;
    const defaults = chosen.filter((entry) => !isDestination(entry));
    const linked = chosen.filter(isDestination);
    if (defaults.length > 0 && (linked.length > 0 || defaults.length > 1)) {
      setMessage({ kind: "error", text: "A schedule can send to several destinations, or to one default channel, not both." });
      return;
    }
    setMessage(null);
    setAdding(true);
    try {
      await api.createSchedule({
        ...params,
        name: name.trim() || `${template} report`,
        channel: defaults[0]?.channel ?? null,
        destination_ids: linked.map((entry) => Number(entry.value.slice(2))),
        time,
        weekday: template === "weekly" ? weekday : null,
      });
      setName("");
      setVersion((value) => value + 1);
    } catch (failure) {
      fail(failure);
    } finally {
      setAdding(false);
    }
  };

  const change = async (action: () => Promise<unknown>) => {
    setMessage(null);
    try {
      await action();
      setVersion((value) => value + 1);
    } catch (failure) {
      fail(failure);
    }
  };

  const copy = async () => {
    if (!report) return;
    try {
      await navigator.clipboard.writeText(report.text);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch {
      setMessage({ kind: "error", text: "Could not copy to the clipboard." });
    }
  };

  const list = schedules.data ?? [];
  const active = list.filter((item) => item.enabled).length;

  return (
    <div className="page fit">
      <PageHeader to="/reports" />
      {message && <p className={message.kind === "ok" ? "notice fit-notice" : "notice error fit-notice"}>{message.text}</p>}

      <div className="fit-grid">
        <section className="fit-col">
          <SectionHead icon={FilePlus2} title="Build a report" />
          <div className="card panel compact fit-scroll">
            <div className="fields one">
              <div>
                <Field label="Period" hint="Daily covers yesterday. Weekly covers the last 7 days.">
                  <Segmented
                    ariaLabel="Period"
                    value={template}
                    onChange={setTemplate}
                    options={(Object.keys(TEMPLATES) as Template[]).map((key) => ({
                      value: key,
                      label: TEMPLATES[key].label,
                      icon: TEMPLATES[key].icon,
                      tip: TEMPLATES[key].tip,
                    }))}
                  />
                </Field>
              </div>
              <div>
                <Field
                  label="Scope"
                  hint="Per destination: each one gets its own ad accounts and template, set on the Destinations page. Custom: choose the accounts and template for this send only."
                >
                  <Segmented
                    ariaLabel="Scope"
                    value={scopeMode}
                    onChange={setScopeMode}
                    options={[
                      { value: "own", label: "Per destination", icon: Users, tip: "Each destination uses its own settings." },
                      { value: "custom", label: "Custom", icon: SlidersHorizontal, tip: "Choose accounts and template for this send." },
                    ]}
                  />
                </Field>
              </div>
              {customMode ? (
                <>
                  <div>
                    <Field
                      label="Ad accounts"
                      hint="Tick several accounts to send one message per account. Tick none to send the total of all accounts."
                    >
                      <AccountPicker accounts={accountList} selected={customAccounts} onChange={setCustomAccounts} />
                    </Field>
                  </div>
                  <div>
                    <Field label="Template" hint="Leave on each destination's own to keep what you set on the Destinations page.">
                      <Select
                        ariaLabel="Template"
                        value={customTemplate}
                        onChange={setCustomTemplate}
                        options={[
                          { value: 0, label: "Each destination's own" },
                          ...templateList.map((entry) => ({ value: entry.id, label: entry.name })),
                        ]}
                      />
                    </Field>
                  </div>
                </>
              ) : (
                <div>
                  <Field
                    label="Fallback ad account"
                    hint="Used only for destinations and default channels that have no ad accounts of their own."
                  >
                    <AccountSelect accounts={accountList} value={accountId} onChange={setAccountId} />
                  </Field>
                </div>
              )}
              <div>
                <Field label="Send to" hint="Tick one or more destinations. Manage them on the Destinations page.">
                  {targets.length > 0 ? (
                    <TargetPicker targets={targets} selected={selected} onChange={setSelected} />
                  ) : (
                    <p className="inline-warn">
                      <TriangleAlert size={15} />
                      <span>
                        No destination yet. <Link to="/destinations">Add one</Link> first.
                      </span>
                    </p>
                  )}
                </Field>
              </div>
            </div>
            <div className="panel-actions">
              <button type="button" className="btn primary grow" disabled={sending || chosen.length === 0} onClick={send}>
                {sending ? <LoaderCircle size={16} className="spin" /> : <Send size={16} />}{" "}
                {chosen.length > 1 ? `Send to ${chosen.length} destinations` : "Send now"}
              </button>
            </div>
            {results && (
              <ul className="send-results">
                {results.map((item, index) => (
                  <li key={`${item.destination_id}-${index}`}>
                    <Result
                      outcome={
                        item.ok
                          ? { status: "ok", message: `${item.name}: sent`, model: "", latency: 0 }
                          : { status: "error", message: `${item.name}: ${item.error ?? "failed"}` }
                      }
                    />
                    {item.parts && item.parts.length > 1 && (
                      <ul className="part-list">
                        {item.parts.map((part, position) => (
                          <li key={position} className={part.ok ? undefined : "bad"}>
                            {part.ok ? "✓" : "✗"} {part.label}
                            {part.error ? `: ${part.error}` : ""}
                          </li>
                        ))}
                      </ul>
                    )}
                  </li>
                ))}
              </ul>
            )}
          </div>
        </section>

        <section className="fit-col">
          <SectionHead icon={FileText} title="Preview" />
          <div className="fit-scroll grow">
            <ChannelPreview
              channel={previewChannel}
              rendered={report?.rendered ?? null}
              email={previewEmail}
              loading={previewing}
              emptyText="The report preview appears here."
              note="The preview skips the AI insight. It is added when sent, only if the template turns it on."
              actions={
                report && (
                  <Tooltip content={copied ? "Copied" : "Copy report text"} align="end">
                    <button type="button" className="icon-button" aria-label="Copy report text" onClick={copy}>
                      {copied ? <Check size={16} /> : <Copy size={16} />}
                    </button>
                  </Tooltip>
                )
              }
            />
          </div>
        </section>

        <section className="fit-col">
          <SectionHead icon={CalendarClock} title="Schedule it" />
          <form className="card panel compact" onSubmit={create}>
            <div className="fields two">
              <div className="span-all">
                <Field label="Name">
                  <input className="select" placeholder="Morning report" value={name} maxLength={80} onChange={(event) => setName(event.target.value)} />
                </Field>
              </div>
              <Field label="Time" hint="Server timezone, 24-hour clock.">
                <input className="select" type="time" value={time} onChange={(event) => setTime(event.target.value)} required />
              </Field>
              {template === "weekly" && (
                <Field label="Day">
                  <Select
                    ariaLabel="Day"
                    value={weekday}
                    onChange={setWeekday}
                    options={WEEKDAYS.map((label, index) => ({ value: index, label }))}
                  />
                </Field>
              )}
            </div>
            <div className="panel-actions">
              <button type="submit" className="btn primary grow" disabled={adding || chosen.length === 0}>
                {adding ? <LoaderCircle size={16} className="spin" /> : <Plus size={16} />} Add schedule
              </button>
            </div>
          </form>

          <SectionHead
            icon={Repeat}
            title="Schedules"
            text={list.length > 0 ? `${active} of ${list.length} active.` : undefined}
          />
          <div className="fit-scroll grow">
            {list.length > 0 ? (
              <ul className="card-grid compact">
                {list.map((item) => {
                  const kind = TEMPLATES[item.template as Template];
                  const linkedDestinations = item.destination_ids
                    .map((id) => destinationList.find((entry) => entry.id === id))
                    .filter((entry): entry is Destination => entry !== undefined);
                  const deliveredBy =
                    linkedDestinations.length === 0
                      ? channelName(item.channel)
                      : linkedDestinations.length === 1
                        ? linkedDestinations[0].name
                        : `${linkedDestinations.length} destinations`;
                  const deliveredChannel = linkedDestinations.length === 1 ? linkedDestinations[0].channel : item.channel;
                  const deliveredTip =
                    linkedDestinations.length > 0 ? linkedDestinations.map((entry) => entry.name).join(", ") : deliveredBy;
                  return (
                    <li key={item.id} className={item.enabled ? "item" : "item off"}>
                      <div className="item-head">
                        <IconTile icon={kind?.icon ?? CalendarClock} tone={item.enabled ? "blue" : "neutral"} />
                        <div className="item-title">
                          <h3 className="truncate">{item.name}</h3>
                          <p className="item-sentence">{when(item)}</p>
                        </div>
                        <div className="item-actions">
                          <Tooltip content={item.enabled ? "Schedule is on. Click to pause" : "Schedule is paused. Click to turn on"} align="end">
                            <Switch
                              checked={item.enabled}
                              label={`Enable ${item.name}`}
                              onChange={(next) => change(() => api.toggleSchedule(item.id, next))}
                            />
                          </Tooltip>
                          <Tooltip content="Delete schedule" align="end">
                            <button
                              type="button"
                              className="icon-button"
                              aria-label={`Delete ${item.name}`}
                              onClick={() => change(() => api.deleteSchedule(item.id))}
                            >
                              <Trash2 size={16} />
                            </button>
                          </Tooltip>
                        </div>
                      </div>

                      <div className="meta-chips">
                        <Tooltip content={kind ? `Covers ${kind.covers}` : "Report template"} align="start">
                          <span className="meta-chip" tabIndex={0}>
                            {kind ? <kind.icon size={14} /> : <CalendarClock size={14} />}
                            {kind?.label ?? titleCase(item.template)}
                          </span>
                        </Tooltip>
                        <Tooltip content={item.account_id ? `Account ID ${item.account_id}` : "Total across every connected account"}>
                          <span className="meta-chip" tabIndex={0}>
                            <BrandIcon name="meta" size="sm" />
                            <span className="truncate">{accountName(item.account_id)}</span>
                          </span>
                        </Tooltip>
                        <Tooltip content={`Delivered to ${deliveredTip}`} align="end">
                          <span className="meta-chip" tabIndex={0}>
                            {linkedDestinations.length > 1 ? <Users size={14} /> : <BrandIcon name={deliveredChannel} size="sm" />}
                            <span className="truncate">{deliveredBy}</span>
                          </span>
                        </Tooltip>
                      </div>

                      <div className="item-foot">
                        <span className="foot-item">
                          <History size={14} />
                          Last run <When value={item.last_run_at} align="start" />
                        </span>
                        <span className="foot-item">
                          <Clock size={14} />
                          {item.time}
                        </span>
                      </div>

                      {item.last_error && (
                        <div className="item-error" role="alert">
                          <TriangleAlert size={15} />
                          <span>{item.last_error}</span>
                        </div>
                      )}
                    </li>
                  );
                })}
              </ul>
            ) : (
              !schedules.loading && (
                <EmptyState
                  icon={CalendarClock}
                  title="No schedules yet"
                  text="Pick destinations, then add a schedule to send automatically."
                />
              )
            )}
          </div>
        </section>
      </div>
    </div>
  );
}
