export function formatNumber(value: number | null | undefined): string {
  if (value === null || value === undefined) return "-";
  return value.toLocaleString("en-US");
}

export function formatPercent(value: number | null | undefined): string {
  if (value === null || value === undefined) return "-";
  return `${(value * 100).toFixed(1)}%`;
}

export function formatMs(value: number | null | undefined): string {
  if (value === null || value === undefined) return "-";
  return `${value.toLocaleString("en-US")} ms`;
}

export function formatMoney(value: number | null | undefined, currency: string | null): string {
  if (value === null || value === undefined) return "-";
  if (!currency) return value.toLocaleString("en-US", { maximumFractionDigits: 2 });
  try {
    return value.toLocaleString("en-US", { style: "currency", currency, maximumFractionDigits: value >= 100 ? 0 : 2 });
  } catch {
    return `${value.toLocaleString("en-US", { maximumFractionDigits: 2 })} ${currency}`;
  }
}

export function formatRatio(value: number | null | undefined): string {
  if (value === null || value === undefined) return "-";
  return `${value.toFixed(2)}x`;
}

export function formatDelta(current: number, previous: number): string | null {
  if (!previous) return null;
  const change = ((current - previous) / previous) * 100;
  return `${change >= 0 ? "+" : ""}${change.toFixed(1)}% vs previous period`;
}

export function formatDateTime(value: string | null): string {
  if (!value) return "Never";
  return new Date(value).toLocaleString("en-US", { dateStyle: "medium", timeStyle: "short" });
}

export function formatRelative(value: string | null): string {
  if (!value) return "Never";
  const seconds = Math.round((Date.now() - new Date(value).getTime()) / 1000);
  const future = seconds < 0;
  const abs = Math.abs(seconds);
  let text: string;
  if (abs < 60) return "Just now";
  if (abs < 3600) text = `${Math.round(abs / 60)} min`;
  else if (abs < 86400) text = `${Math.round(abs / 3600)} h`;
  else if (abs < 2592000) text = `${Math.round(abs / 86400)} d`;
  else return formatDateTime(value);
  return future ? `in ${text}` : `${text} ago`;
}
