import {
  BellOff,
  BellRing,
  CircleAlert,
  Clock,
  History,
  LoaderCircle,
  Plus,
  Timer,
  Trash2,
  Activity,
  MessageSquareText,
} from "lucide-react";
import { type FormEvent, useEffect, useMemo, useState } from "react";
import { api, type AdAccount, type AlertPreview, type AlertRule, type AlertRuleInput, type Channel } from "../api";
import {
  COOLDOWNS,
  METRIC_KEYS,
  OPERATORS,
  PERIODS,
  cooldownLabel,
  formatMetricValue,
  metricMeta,
  unitFor,
} from "../alertMeta";
import { AccountSelect } from "../components/AccountSelect";
import { ChannelPreview } from "../components/ChannelPreview";
import { ChannelSelect } from "../components/ChannelSelect";
import { EmptyState } from "../components/EmptyState";
import { Field } from "../components/Field";
import { BrandIcon, IconTile } from "../components/IconTile";
import { PageHeader } from "../components/PageHeader";
import { Segmented } from "../components/Segmented";
import { Select } from "../components/Select";
import { SectionHead } from "../components/SectionHead";
import { Switch } from "../components/Switch";
import { Tooltip } from "../components/Tooltip";
import { formatDateTime } from "../format";
import { useAsync } from "../hooks";

function currencyFor(accounts: AdAccount[], accountId: string | null): string | null {
  const scoped = accountId ? accounts.filter((account) => account.id === accountId) : accounts;
  const currencies = new Set(scoped.map((account) => account.currency).filter(Boolean));
  return currencies.size === 1 ? [...currencies][0] : null;
}

function channelLabel(channels: Channel[], name: string): string {
  return channels.find((channel) => channel.name === name)?.label ?? name;
}

function accountLabel(accounts: AdAccount[], id: string | null): string {
  if (!id) return "All accounts";
  return accounts.find((account) => account.id === id)?.name ?? id;
}

interface RuleCardProps {
  rule: AlertRule;
  accounts: AdAccount[];
  channels: Channel[];
  onToggle: (rule: AlertRule, enabled: boolean) => void;
  onDelete: (rule: AlertRule) => void;
}

function RuleCard({ rule, accounts, channels, onToggle, onDelete }: RuleCardProps) {
  const meta = metricMeta(rule.metric);
  const currency = currencyFor(accounts, rule.account_id);
  const operator = OPERATORS[rule.operator];
  const period = PERIODS[rule.period];
  const channel = channels.find((entry) => entry.name === rule.channel);

  return (
    <li className={rule.enabled ? "item" : "item off"}>
      <div className="item-head">
        <IconTile icon={meta.icon} tone={rule.enabled ? "amber" : "neutral"} />
        <div className="item-title">
          <h3 className="truncate">{rule.name}</h3>
          <p className="item-sentence">
            <Tooltip content={meta.description} align="start">
              <strong tabIndex={0}>{meta.short}</strong>
            </Tooltip>{" "}
            {operator.verb} <strong>{formatMetricValue(rule.threshold, meta, currency)}</strong>
          </p>
        </div>
        <div className="item-actions">
          <Tooltip content={rule.enabled ? "Rule is on. Click to pause" : "Rule is paused. Click to turn on"} align="end">
            <Switch checked={rule.enabled} label={`Enable ${rule.name}`} onChange={(next) => onToggle(rule, next)} />
          </Tooltip>
          <Tooltip content="Delete rule" align="end">
            <button type="button" className="icon-button" aria-label={`Delete ${rule.name}`} onClick={() => onDelete(rule)}>
              <Trash2 size={16} />
            </button>
          </Tooltip>
        </div>
      </div>

      <div className="meta-chips">
        <Tooltip content={period.tip} align="start">
          <span className="meta-chip" tabIndex={0}>
            <period.icon size={14} />
            {period.label}
          </span>
        </Tooltip>
        <Tooltip content={rule.account_id ? `Account ID ${rule.account_id}` : "Checks the total across every connected account"}>
          <span className="meta-chip" tabIndex={0}>
            <BrandIcon name="meta" size="sm" />
            <span className="truncate">{accountLabel(accounts, rule.account_id)}</span>
          </span>
        </Tooltip>
        <Tooltip content={channel && !channel.configured ? `${channelLabel(channels, rule.channel)} is not configured, so this alert cannot be delivered` : `Notifies via ${channelLabel(channels, rule.channel)}`}>
          <span className={channel && !channel.configured ? "meta-chip warn" : "meta-chip"} tabIndex={0}>
            <BrandIcon name={rule.channel} size="sm" />
            {channelLabel(channels, rule.channel)}
          </span>
        </Tooltip>
        <Tooltip content={`After firing, this rule stays quiet for ${cooldownLabel(rule.cooldown_hours)}`} align="end">
          <span className="meta-chip" tabIndex={0}>
            <Timer size={14} />
            {cooldownLabel(rule.cooldown_hours)}
          </span>
        </Tooltip>
      </div>

      <div className="item-foot">
        <Tooltip content="Value measured at the most recent check" align="start">
          <span className="foot-item" tabIndex={0}>
            <Activity size={14} />
            {rule.last_value !== null ? formatMetricValue(rule.last_value, meta, currency) : "Not checked yet"}
          </span>
        </Tooltip>
        <Tooltip content="When this rule last sent a notification" align="end">
          <span className="foot-item" tabIndex={0}>
            <History size={14} />
            {formatDateTime(rule.last_triggered_at)}
          </span>
        </Tooltip>
      </div>

      {rule.last_error && (
        <div className="item-error" role="alert">
          <CircleAlert size={15} />
          <span>{rule.last_error}</span>
        </div>
      )}
    </li>
  );
}

export function Alerts() {
  const accounts = useAsync(() => api.adsAccounts(), []);
  const channels = useAsync(() => api.channels(), []);
  const [version, setVersion] = useState(0);
  const rules = useAsync(() => api.alertRules(), [version]);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [thresholdText, setThresholdText] = useState("0");
  const [form, setForm] = useState<AlertRuleInput>({
    name: "",
    account_id: null,
    metric: "spend",
    operator: "gt",
    threshold: 0,
    period: "yesterday",
    channel: "telegram",
    cooldown_hours: 24,
  });
  const update = (patch: Partial<AlertRuleInput>) => setForm((current) => ({ ...current, ...patch }));

  const accountList = accounts.data?.accounts ?? [];
  const channelList = channels.data ?? [];
  const meta = metricMeta(form.metric);
  const currency = useMemo(() => currencyFor(accountList, form.account_id), [accountList, form.account_id]);
  const unit = unitFor(meta, currency);
  const parsed = thresholdText.trim() === "" ? NaN : Number(thresholdText);
  const valid = Number.isFinite(parsed);
  const stored = valid ? parsed / meta.scale : 0;
  const selectedChannel = channelList.find((channel) => channel.name === form.channel);

  const [preview, setPreview] = useState<AlertPreview | null>(null);
  const [previewing, setPreviewing] = useState(false);
  const [previewError, setPreviewError] = useState<string | null>(null);

  useEffect(() => {
    if (!valid) return;
    let active = true;
    setPreviewing(true);
    const timer = window.setTimeout(() => {
      api
        .previewAlert({
          name: form.name.trim() || `${meta.short} alert`,
          account_id: form.account_id,
          metric: form.metric,
          operator: form.operator,
          threshold: stored,
          period: form.period,
        })
        .then((result) => {
          if (!active) return;
          setPreview(result);
          setPreviewError(null);
        })
        .catch((failure: unknown) => {
          if (active) setPreviewError(failure instanceof Error ? failure.message : "Preview failed");
        })
        .finally(() => {
          if (active) setPreviewing(false);
        });
    }, 300);
    return () => {
      active = false;
      window.clearTimeout(timer);
    };
  }, [form.name, form.account_id, form.metric, form.operator, form.period, meta.short, stored, valid]);

  const create = async (event: FormEvent) => {
    event.preventDefault();
    if (!valid) return;
    setError(null);
    setSaving(true);
    try {
      await api.createAlertRule({
        ...form,
        threshold: stored,
        name: form.name.trim() || `${meta.short} alert`,
      });
      update({ name: "" });
      setVersion((value) => value + 1);
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : "Request failed");
    } finally {
      setSaving(false);
    }
  };

  const run = async (action: () => Promise<unknown>) => {
    setError(null);
    try {
      await action();
      setVersion((value) => value + 1);
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : "Request failed");
    }
  };

  const list = rules.data ?? [];
  const activeCount = list.filter((rule) => rule.enabled).length;

  return (
    <div className="page">
      <PageHeader to="/alerts" />

      <div className="split trio">
        <aside className="split-side sticky">
          <SectionHead icon={Plus} title="New rule" text="Get notified when a metric crosses your limit." />
          <form className="card panel" onSubmit={create}>
            <div className="preview-box" aria-live="polite">
              <IconTile icon={meta.icon} size="sm" tone="amber" />
              <p>
                Notify me on <strong>{channelLabel(channelList, form.channel)}</strong> when <strong>{meta.short}</strong>{" "}
                {OPERATORS[form.operator].verb}{" "}
                <strong>{valid ? `${parsed.toLocaleString("en-US")}${unit ? ` ${unit}` : ""}` : "…"}</strong>{" "}
                for {PERIODS[form.period].phrase} on <strong>{accountLabel(accountList, form.account_id)}</strong>.
              </p>
            </div>

            <div className="fields two">
              <div className="span-all">
                <Field label="Name" hint="Shown in the notification. Leave empty to use the metric name.">
                  <input
                    className="select"
                    placeholder={`${meta.short} alert`}
                    value={form.name}
                    maxLength={80}
                    onChange={(event) => update({ name: event.target.value })}
                  />
                </Field>
              </div>

              <div className="span-all">
                <Field label="Metric" hint={meta.description}>
                  <Select
                    ariaLabel="Metric"
                    value={form.metric}
                    onChange={(metric) => update({ metric })}
                    options={METRIC_KEYS.map((key) => {
                      const entry = metricMeta(key);
                      return {
                        value: key,
                        label: entry.label,
                        description: entry.description,
                        icon: <IconTile icon={entry.icon} size="sm" />,
                      };
                    })}
                  />
                </Field>
              </div>

              <Field label="Condition">
                <Segmented
                  ariaLabel="Condition"
                  value={form.operator}
                  onChange={(operator) => update({ operator })}
                  options={(Object.keys(OPERATORS) as AlertRuleInput["operator"][]).map((key) => ({
                    value: key,
                    label: OPERATORS[key].label,
                    icon: OPERATORS[key].icon,
                    tip: OPERATORS[key].tip,
                  }))}
                />
              </Field>

              <Field label="Threshold" hint="The limit that triggers the alert.">
                <div className={valid ? "affix" : "affix invalid"}>
                  <input
                    type="number"
                    inputMode="decimal"
                    step="any"
                    aria-label="Threshold"
                    aria-invalid={!valid}
                    value={thresholdText}
                    onChange={(event) => setThresholdText(event.target.value)}
                  />
                  {unit && <span className="affix-unit">{unit}</span>}
                </div>
              </Field>

              <div className="span-all">
                <Field label="Period" hint="Which time window the metric is measured over.">
                  <Segmented
                    ariaLabel="Period"
                    value={form.period}
                    onChange={(period) => update({ period })}
                    options={(Object.keys(PERIODS) as AlertRuleInput["period"][]).map((key) => ({
                      value: key,
                      label: PERIODS[key].label,
                      icon: PERIODS[key].icon,
                      tip: PERIODS[key].tip,
                    }))}
                  />
                </Field>
              </div>

              <div className="span-all">
                <Field label="Ad account" hint="Check one account, or the total of all of them.">
                  <AccountSelect accounts={accountList} value={form.account_id} onChange={(value) => update({ account_id: value })} />
                </Field>
              </div>

              <Field label="Notify via" hint="Where the alert message is delivered.">
                <ChannelSelect channels={channelList} value={form.channel} onChange={(value) => update({ channel: value })} />
              </Field>

              <Field label="Repeat at most every" hint="After an alert fires, it will not fire again until this time has passed.">
                <Select
                  ariaLabel="Cooldown"
                  value={form.cooldown_hours}
                  onChange={(cooldown_hours) => update({ cooldown_hours })}
                  options={COOLDOWNS.map((hours) => ({
                    value: hours,
                    label: cooldownLabel(hours),
                    icon: <IconTile icon={Clock} size="sm" />,
                  }))}
                />
              </Field>
            </div>

            {selectedChannel && !selectedChannel.configured && (
              <p className="inline-warn">
                <CircleAlert size={15} />
                {selectedChannel.label} is not configured yet. The rule will be saved but cannot send until you set it up.
              </p>
            )}

            <div className="panel-actions">
              <button type="submit" className="btn primary grow" disabled={!valid || saving}>
                {saving ? <LoaderCircle size={16} className="spin" /> : <Plus size={16} />}
                Add rule
              </button>
            </div>
          </form>
          {error && <p className="notice error">{error}</p>}
        </aside>

        <aside className="split-preview sticky">
          <SectionHead icon={MessageSquareText} title="Preview" text="The message you will receive when this rule fires." />
          <ChannelPreview
            channel={form.channel}
            rendered={preview?.rendered ?? null}
            email={preview?.email ?? null}
            loading={previewing}
            emptyText="Enter a threshold to see the message."
            note={
              preview &&
              (preview.sample
                ? "No data in this period yet, so the message uses a sample value just past your limit."
                : `Right now ${meta.short} is ${formatMetricValue(preview.value ?? 0, meta, currency)}. ${
                    preview.breached ? "This rule would fire." : "This rule would stay quiet."
                  }`)
            }
          />
          {previewError && <p className="notice error">{previewError}</p>}
        </aside>

        <section className="split-main">
          <SectionHead
            icon={BellRing}
            title="Your rules"
            text={list.length > 0 ? `${activeCount} of ${list.length} active. Checked after every Meta sync.` : "Checked after every Meta sync."}
          />
          {list.length > 0 ? (
            <ul className="card-grid">
              {list.map((rule) => (
                <RuleCard
                  key={rule.id}
                  rule={rule}
                  accounts={accountList}
                  channels={channelList}
                  onToggle={(target, enabled) => run(() => api.toggleAlertRule(target.id, enabled))}
                  onDelete={(target) => run(() => api.deleteAlertRule(target.id))}
                />
              ))}
            </ul>
          ) : (
            !rules.loading && (
              <EmptyState icon={BellOff} title="No alert rules yet" text="Create your first rule and Heimdall will message you when a metric crosses your limit." />
            )
          )}
        </section>
      </div>
    </div>
  );
}
