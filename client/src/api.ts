export type Range = "24h" | "7d" | "14d" | "30d";

export interface SeriesPoint {
  bucket: string;
  tokens: number;
  requests: number;
}

export interface LlmDashboard {
  range: Range;
  total_tokens: number;
  total_requests: number;
  success_rate: number | null;
  cache_hit_rate: number | null;
  avg_latency_ms: number | null;
  series: SeriesPoint[];
}

export interface Provider {
  name: string;
  label: string;
  configured: boolean;
  model: string;
  priority: number | null;
}

export interface TestResult {
  provider: string;
  model: string;
  text: string;
  latency_ms: number;
  input_tokens: number;
  output_tokens: number;
}

export interface AdAccount {
  id: string;
  name: string;
  currency: string;
  timezone: string;
  status: string;
  last_synced_at: string | null;
  last_error: string | null;
  connection_id: number | null;
  connection_name: string | null;
}

export interface MetaConnection {
  id: number;
  name: string;
  enabled: boolean;
  preview: string;
  accounts: number;
  created_at: string | null;
  last_synced_at: string | null;
  last_error: string | null;
}

export interface MetaConnections {
  connections: MetaConnection[];
}

export interface MetaConnectionTest {
  message: string;
  accounts: number;
  connections: MetaConnection[];
}

export interface MetaConnectionPatch {
  name?: string;
  token?: string;
  enabled?: boolean;
}

export interface AdAccounts {
  configured: boolean;
  syncing: boolean;
  accounts: AdAccount[];
}

export interface AdMetrics {
  spend: number;
  impressions: number;
  clicks: number;
  conversions: number;
  ctr: number | null;
  cpc: number | null;
  cpm: number | null;
  cpa: number | null;
  roas: number | null;
}

export interface AdCampaign extends AdMetrics {
  account_id: string;
  campaign_id: string;
  name: string;
  status: string | null;
  objective: string | null;
  daily_budget: number | null;
}

export interface AdSeriesPoint {
  day: string;
  spend: number;
  clicks: number;
  conversions: number;
}

export interface AdsInsights {
  range: Range;
  start: string;
  end: string;
  currency: string | null;
  totals: AdMetrics;
  previous: AdMetrics;
  series: AdSeriesPoint[];
  campaigns: AdCampaign[];
}

export interface ChartSeries {
  key: string;
  name: string;
  unit: "money" | "count" | "percent" | "ratio";
}

export interface ChartSpec {
  type: "line" | "bar";
  title: string;
  level: "day" | "campaign" | "account";
  start: string;
  end: string;
  currency: string | null;
  series: ChartSeries[];
  data: Array<Record<string, string | number | null>>;
}

export interface ChatMessage {
  id: number;
  role: "user" | "assistant";
  content: string;
  run_id: string | null;
  created_at: string;
  charts?: ChartSpec[];
}

export interface Conversation {
  id: string;
  title: string;
  updated_at: string;
}

export interface ConversationDetail extends Conversation {
  messages: ChatMessage[];
}

export interface ChatReply {
  conversation_id: string;
  message: ChatMessage;
}

export interface Run {
  id: string;
  created_at: string;
  input: string;
  status: string;
  steps: number;
  total_tokens: number;
  duration_ms: number;
  error: string | null;
}

export interface ToolCallLog {
  step: number;
  tool_name: string;
  arguments: string;
  ok: boolean;
  error_code: string | null;
  latency_ms: number;
  result: string;
}

export interface ModelRequest {
  created_at: string;
  provider: string;
  model: string;
  input_tokens: number;
  output_tokens: number;
  cached_tokens: number;
  latency_ms: number;
  ok: boolean;
  error_code: string | null;
}

export interface RunDetail extends Run {
  output: string;
  tool_calls: ToolCallLog[];
  model_requests: ModelRequest[];
}

export interface Channel {
  name: string;
  label: string;
  configured: boolean;
}

export interface Rendered {
  subject: string | null;
  body: string;
  clipped: boolean;
  limit: number | null;
}

export type RenderedSet = Record<"telegram" | "slack" | "zalo" | "email", Rendered>;

export interface EmailContext {
  from: string | null;
  to: string[];
}

export interface Report {
  title: string;
  text: string;
  rendered: RenderedSet;
  email: EmailContext;
}

export interface AlertDraft {
  name: string;
  account_id: string | null;
  metric: string;
  operator: "gt" | "lt";
  threshold: number;
  period: "today" | "yesterday" | "last_7d";
}

export interface AlertPreview {
  title: string;
  text: string;
  value: number | null;
  sample: boolean;
  breached: boolean;
  rendered: RenderedSet;
  email: EmailContext;
}

export interface ReportParams {
  template: "daily" | "weekly";
  account_id: string | null;
}

export interface Schedule {
  id: number;
  name: string;
  template: string;
  account_id: string | null;
  channel: string;
  time: string;
  weekday: number | null;
  enabled: boolean;
  last_run_at: string | null;
  last_error: string | null;
}

export interface ScheduleInput extends ReportParams {
  name: string;
  channel: string;
  time: string;
  weekday: number | null;
}

export interface AlertRuleInput {
  name: string;
  account_id: string | null;
  metric: string;
  operator: "gt" | "lt";
  threshold: number;
  period: "today" | "yesterday" | "last_7d";
  channel: string;
  cooldown_hours: number;
}

export interface AlertRule extends AlertRuleInput {
  id: number;
  enabled: boolean;
  last_triggered_at: string | null;
  last_value: number | null;
  last_error: string | null;
}

export type ApprovalStatus = "pending" | "executed" | "failed" | "rejected" | "expired";

export interface Approval {
  id: number;
  created_at: string;
  expires_at: string | null;
  run_id: string | null;
  tool_name: string;
  account_id: string;
  campaign_id: string;
  campaign_name: string;
  summary: string;
  reason: string;
  status: ApprovalStatus;
  decided_at: string | null;
  result: string | null;
}

export interface Approvals {
  pending: number;
  writes_enabled: boolean;
  items: Approval[];
}

export type SettingSource = "app" | "default";

export interface SettingField {
  key: string;
  label: string;
  hint: string;
  kind: string;
  secret: boolean;
  placeholder: string;
  is_set: boolean;
  value: string | null;
  preview: string | null;
  source: SettingSource;
}

export interface SettingGroup {
  key: string;
  label: string;
  description: string;
  section: string;
  testable: boolean;
  configured: boolean | null;
  priority: number | null;
  fields: SettingField[];
}

export interface SettingsAccess {
  editable: boolean;
  requires_token: boolean;
  reason: string | null;
}

export interface SettingsPayload {
  access: SettingsAccess;
  groups: SettingGroup[];
}

export interface GroupTestResult {
  message: string;
  model: string;
  latency_ms: number;
}

export class ApiError extends Error {}

const TOKEN_KEY = "heimdall.adminToken";

export function getAdminToken(): string {
  try {
    return window.sessionStorage.getItem(TOKEN_KEY) ?? "";
  } catch {
    return "";
  }
}

export function setAdminToken(token: string): void {
  try {
    if (token) window.sessionStorage.setItem(TOKEN_KEY, token);
    else window.sessionStorage.removeItem(TOKEN_KEY);
  } catch {
    return;
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const body = typeof init?.body === "string" ? JSON.parse(init.body) : undefined;
  let result: { status: number; body: unknown };
  try {
    result = await window.heimdall.invoke(init?.method ?? "GET", path, body);
  } catch {
    throw new ApiError("Cannot reach the Heimdall backend.");
  }
  if (result.status >= 400) {
    let message = "Request failed";
    const detail = (result.body as { detail?: unknown } | null)?.detail;
    if (typeof detail === "string") message = detail;
    throw new ApiError(message);
  }
  if (result.status === 204) return undefined as T;
  return result.body as T;
}

export const api = {
  llmDashboard: (range: Range) => request<LlmDashboard>(`/dashboard/llm?range=${range}`),
  adsAccounts: () => request<AdAccounts>("/ads/accounts"),
  adsSync: () => request<AdAccounts>("/ads/sync", { method: "POST" }),
  adsInsights: (range: Range, accountId: string | null) =>
    request<AdsInsights>(`/ads/insights?range=${range}${accountId ? `&account_id=${encodeURIComponent(accountId)}` : ""}`),
  conversations: () => request<Conversation[]>("/chat/conversations"),
  conversation: (id: string) => request<ConversationDetail>(`/chat/conversations/${id}`),
  deleteConversation: (id: string) => request<void>(`/chat/conversations/${id}`, { method: "DELETE" }),
  sendMessage: (text: string, conversationId: string | null) =>
    request<ChatReply>("/chat/messages", {
      method: "POST",
      body: JSON.stringify({ text, conversation_id: conversationId }),
    }),
  runs: () => request<Run[]>("/logs/runs?limit=100"),
  run: (id: string) => request<RunDetail>(`/logs/runs/${id}`),
  channels: () => request<Channel[]>("/channels"),
  testChannel: (name: string) => request<void>(`/channels/${name}/test`, { method: "POST" }),
  previewReport: (params: ReportParams) =>
    request<Report>("/reports/preview", { method: "POST", body: JSON.stringify(params) }),
  sendReport: (params: ReportParams, channel: string) =>
    request<void>("/reports/send", { method: "POST", body: JSON.stringify({ ...params, channel }) }),
  schedules: () => request<Schedule[]>("/reports/schedules"),
  createSchedule: (input: ScheduleInput) =>
    request<Schedule>("/reports/schedules", { method: "POST", body: JSON.stringify(input) }),
  toggleSchedule: (id: number, enabled: boolean) =>
    request<Schedule>(`/reports/schedules/${id}`, { method: "PATCH", body: JSON.stringify({ enabled }) }),
  deleteSchedule: (id: number) => request<void>(`/reports/schedules/${id}`, { method: "DELETE" }),
  previewAlert: (draft: AlertDraft) =>
    request<AlertPreview>("/alerts/preview", { method: "POST", body: JSON.stringify(draft) }),
  alertRules: () => request<AlertRule[]>("/alerts/rules"),
  createAlertRule: (input: AlertRuleInput) =>
    request<AlertRule>("/alerts/rules", { method: "POST", body: JSON.stringify(input) }),
  toggleAlertRule: (id: number, enabled: boolean) =>
    request<AlertRule>(`/alerts/rules/${id}`, { method: "PATCH", body: JSON.stringify({ enabled }) }),
  deleteAlertRule: (id: number) => request<void>(`/alerts/rules/${id}`, { method: "DELETE" }),
  approvals: (status: ApprovalStatus | null) =>
    request<Approvals>(`/approvals${status ? `?status=${status}` : ""}`),
  approve: (id: number) => request<Approval>(`/approvals/${id}/approve`, { method: "POST" }),
  reject: (id: number) => request<Approval>(`/approvals/${id}/reject`, { method: "POST" }),
  settings: () => request<SettingsPayload>("/settings"),
  saveSettings: (group: string, values: Record<string, string>, clear: string[]) =>
    request<SettingsPayload>(`/settings/${group}`, { method: "PUT", body: JSON.stringify({ values, clear }) }),
  testSettings: (group: string) => request<GroupTestResult>(`/settings/${group}/test`, { method: "POST" }),
  metaConnections: () => request<MetaConnections>("/meta/connections"),
  createMetaConnection: (name: string, token: string) =>
    request<MetaConnections>("/meta/connections", { method: "POST", body: JSON.stringify({ name, token }) }),
  updateMetaConnection: (id: number, patch: MetaConnectionPatch) =>
    request<MetaConnections>(`/meta/connections/${id}`, { method: "PUT", body: JSON.stringify(patch) }),
  deleteMetaConnection: (id: number) => request<MetaConnections>(`/meta/connections/${id}`, { method: "DELETE" }),
  testMetaConnection: (id: number) => request<MetaConnectionTest>(`/meta/connections/${id}/test`, { method: "POST" }),
  providers: () => request<Provider[]>("/llm/providers"),
  testProvider: (provider: string) =>
    request<TestResult>("/llm/test", {
      method: "POST",
      body: JSON.stringify({ provider }),
    }),
};
