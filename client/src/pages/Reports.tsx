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
  Trash2,
  TriangleAlert,
} from "lucide-react";
import { type FormEvent, useEffect, useState } from "react";
import { api, type Report, type Schedule } from "../api";
import { AccountSelect } from "../components/AccountSelect";
import { ChannelPreview } from "../components/ChannelPreview";
import { ChannelSelect } from "../components/ChannelSelect";
import { EmptyState } from "../components/EmptyState";
import { Field } from "../components/Field";
import { BrandIcon, IconTile } from "../components/IconTile";
import { PageHeader } from "../components/PageHeader";
import { Segmented } from "../components/Segmented";
import { SectionHead } from "../components/SectionHead";
import { Select } from "../components/Select";
import { Switch } from "../components/Switch";
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

function when(item: Schedule): string {
  return item.weekday !== null ? `Every ${WEEKDAYS[item.weekday] ?? "week"} at ${item.time}` : `Every day at ${item.time}`;
}

export function Reports() {
  const accounts = useAsync(() => api.adsAccounts(), []);
  const channels = useAsync(() => api.channels(), []);
  const [version, setVersion] = useState(0);
  const schedules = useAsync(() => api.schedules(), [version]);

  const [template, setTemplate] = useState<Template>("daily");
  const [accountId, setAccountId] = useState<string | null>(null);
  const [channel, setChannel] = useState("telegram");
  const [report, setReport] = useState<Report | null>(null);
  const [sending, setSending] = useState(false);
  const [previewing, setPreviewing] = useState(false);
  const [message, setMessage] = useState<{ kind: "ok" | "error"; text: string } | null>(null);
  const [copied, setCopied] = useState(false);

  const [name, setName] = useState("");
  const [time, setTime] = useState("08:00");
  const [weekday, setWeekday] = useState(0);
  const [adding, setAdding] = useState(false);

  const params = { template, account_id: accountId };
  const channelList = channels.data ?? [];
  const accountList = accounts.data?.accounts ?? [];
  const selectedChannel = channelList.find((entry) => entry.name === channel);
  const accountName = (id: string | null) => (id ? (accountList.find((entry) => entry.id === id)?.name ?? id) : "All accounts");
  const channelName = (id: string) => channelList.find((entry) => entry.name === id)?.label ?? id;
  const fail = (failure: unknown) =>
    setMessage({ kind: "error", text: failure instanceof Error ? failure.message : "Request failed" });

  useEffect(() => {
    let active = true;
    setPreviewing(true);
    api
      .previewReport({ template, account_id: accountId })
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
  }, [template, accountId]);

  const send = async () => {
    setSending(true);
    setMessage(null);
    try {
      await api.sendReport(params, channel);
      setMessage({ kind: "ok", text: `Report sent to ${channelName(channel)}.` });
    } catch (failure) {
      fail(failure);
    } finally {
      setSending(false);
    }
  };

  const create = async (event: FormEvent) => {
    event.preventDefault();
    setMessage(null);
    setAdding(true);
    try {
      await api.createSchedule({
        ...params,
        name: name.trim() || `${template} report`,
        channel,
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
    <div className="page">
      <PageHeader to="/reports" />

      <div className="split trio">
        <aside className="split-side">
          <SectionHead icon={FilePlus2} title="Build a report" text="Pick what to cover and send it to a channel. The preview updates as you choose." />
          <div className="card panel">
            <div className="fields">
              <div className="span-all">
                <Field label="Template" hint="Daily covers yesterday. Weekly covers the last 7 days.">
                  <Segmented
                    ariaLabel="Template"
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
              <div className="span-all">
                <Field label="Ad account" hint="Report on one account, or the total of all of them.">
                  <AccountSelect accounts={accountList} value={accountId} onChange={setAccountId} />
                </Field>
              </div>
              <div className="span-all">
                <Field label="Channel" hint="Where the report is delivered when you send it.">
                  <ChannelSelect channels={channelList} value={channel} onChange={setChannel} />
                </Field>
              </div>
            </div>
            {selectedChannel && !selectedChannel.configured && (
              <p className="inline-warn">
                <TriangleAlert size={15} />
                {selectedChannel.label} is not configured yet, so sending will fail.
              </p>
            )}
            <div className="panel-actions">
              <button type="button" className="btn primary grow" disabled={sending} onClick={send}>
                {sending ? <LoaderCircle size={16} className="spin" /> : <Send size={16} />} Send now
              </button>
            </div>
          </div>

          <div className="gap" />
          <SectionHead
            icon={CalendarClock}
            title="Schedule it"
            text="Uses the template, account and channel above. Times follow the server timezone."
          />
          <form className="card panel" onSubmit={create}>
            <div className="fields">
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
              <button type="submit" className="btn primary grow" disabled={adding}>
                {adding ? <LoaderCircle size={16} className="spin" /> : <Plus size={16} />} Add schedule
              </button>
            </div>
          </form>
          {message && <p className={message.kind === "ok" ? "notice" : "notice error"}>{message.text}</p>}
        </aside>

        <aside className="split-preview sticky">
          <SectionHead icon={FileText} title="Preview" text="The message as the selected channel will show it." />
          <ChannelPreview
            channel={channel}
            rendered={report?.rendered ?? null}
            email={report?.email ?? null}
            loading={previewing}
            emptyText="The report preview appears here."
            note="The preview does not call the language model. The AI insight is added when the report is sent."
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
        </aside>

        <div className="split-main">
          <section className="section">
            <SectionHead
              icon={Repeat}
              title="Schedules"
              text={list.length > 0 ? `${active} of ${list.length} active.` : "Reports sent automatically at a set time."}
            />
            {list.length > 0 ? (
              <ul className="card-grid">
                {list.map((item) => {
                  const kind = TEMPLATES[item.template as Template];
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
                        <Tooltip content={`Delivered via ${channelName(item.channel)}`} align="end">
                          <span className="meta-chip" tabIndex={0}>
                            <BrandIcon name={item.channel} size="sm" />
                            {channelName(item.channel)}
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
                  text="Choose a template, account and channel, then add a schedule to send it automatically."
                />
              )
            )}
          </section>
        </div>
      </div>
    </div>
  );
}
