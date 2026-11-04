import {
  CalendarDays,
  CalendarRange,
  Coins,
  Eye,
  Gauge,
  MousePointerClick,
  Percent,
  ReceiptText,
  Sun,
  Target,
  TrendingDown,
  TrendingUp,
  Wallet,
  type LucideIcon,
} from "lucide-react";
import type { AlertRuleInput } from "./api";
import { formatMoney } from "./format";

export type MetricKind = "money" | "count" | "percent" | "ratio";

export interface MetricMeta {
  label: string;
  short: string;
  icon: LucideIcon;
  kind: MetricKind;
  scale: number;
  description: string;
}

export const METRICS: Record<string, MetricMeta> = {
  spend: {
    label: "Spend",
    short: "Spend",
    icon: Wallet,
    kind: "money",
    scale: 1,
    description: "Total amount spent on ads in the period.",
  },
  impressions: {
    label: "Impressions",
    short: "Impressions",
    icon: Eye,
    kind: "count",
    scale: 1,
    description: "How many times your ads were shown.",
  },
  clicks: {
    label: "Clicks",
    short: "Clicks",
    icon: MousePointerClick,
    kind: "count",
    scale: 1,
    description: "Total clicks on your ads.",
  },
  conversions: {
    label: "Conversions",
    short: "Conversions",
    icon: Target,
    kind: "count",
    scale: 1,
    description: "Results such as purchases or leads.",
  },
  ctr: {
    label: "Click-through rate (CTR)",
    short: "CTR",
    icon: Percent,
    kind: "percent",
    scale: 100,
    description: "Clicks divided by impressions. Higher means the ad is more engaging.",
  },
  cpc: {
    label: "Cost per click (CPC)",
    short: "CPC",
    icon: Coins,
    kind: "money",
    scale: 1,
    description: "Average amount paid for each click.",
  },
  cpm: {
    label: "Cost per 1,000 views (CPM)",
    short: "CPM",
    icon: Gauge,
    kind: "money",
    scale: 1,
    description: "Average cost to show your ad 1,000 times.",
  },
  cpa: {
    label: "Cost per result (CPA)",
    short: "CPA",
    icon: ReceiptText,
    kind: "money",
    scale: 1,
    description: "Average amount paid for each conversion. Lower is better.",
  },
  roas: {
    label: "Return on ad spend (ROAS)",
    short: "ROAS",
    icon: TrendingUp,
    kind: "ratio",
    scale: 1,
    description: "Revenue earned for every 1 spent. 3x means 3 back per 1 spent.",
  },
};

export const METRIC_KEYS = Object.keys(METRICS);

export function metricMeta(key: string): MetricMeta {
  return (
    METRICS[key] ?? { label: key, short: key, icon: Gauge, kind: "count", scale: 1, description: "" }
  );
}

export const OPERATORS: Record<AlertRuleInput["operator"], { label: string; verb: string; icon: LucideIcon; tip: string }> = {
  gt: { label: "Goes above", verb: "is above", icon: TrendingUp, tip: "Alert when the value is higher than the threshold." },
  lt: { label: "Drops below", verb: "is below", icon: TrendingDown, tip: "Alert when the value is lower than the threshold." },
};

export const PERIODS: Record<AlertRuleInput["period"], { label: string; phrase: string; icon: LucideIcon; tip: string }> = {
  today: { label: "Today", phrase: "today", icon: Sun, tip: "Data from midnight until the latest sync." },
  yesterday: { label: "Yesterday", phrase: "yesterday", icon: CalendarDays, tip: "The previous full day." },
  last_7d: { label: "7 days", phrase: "the last 7 days", icon: CalendarRange, tip: "Combined total of the last 7 days." },
};

export const COOLDOWNS = [1, 3, 6, 12, 24, 48, 168];

export function cooldownLabel(hours: number): string {
  if (hours % 168 === 0) return hours === 168 ? "1 week" : `${hours / 168} weeks`;
  if (hours % 24 === 0) return hours === 24 ? "1 day" : `${hours / 24} days`;
  return hours === 1 ? "1 hour" : `${hours} hours`;
}

export function unitFor(meta: MetricMeta, currency: string | null): string {
  switch (meta.kind) {
    case "money":
      return currency ?? "";
    case "percent":
      return "%";
    case "ratio":
      return "x";
    default:
      return "";
  }
}

export function formatMetricValue(value: number, meta: MetricMeta, currency: string | null): string {
  switch (meta.kind) {
    case "money":
      return formatMoney(value, currency);
    case "percent":
      return `${(value * meta.scale).toLocaleString("en-US", { maximumFractionDigits: 2 })}%`;
    case "ratio":
      return `${value.toLocaleString("en-US", { maximumFractionDigits: 2 })}x`;
    default:
      return value.toLocaleString("en-US", { maximumFractionDigits: 2 });
  }
}
