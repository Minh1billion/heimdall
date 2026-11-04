import {
  Bell,
  Building2,
  ChartNoAxesColumn,
  FileText,
  House,
  MessageCircle,
  Rows3,
  Settings,
  ShieldCheck,
  type LucideIcon,
} from "lucide-react";

export type NavGroup = "Workspace" | "Ads" | "System";

export interface NavItem {
  to: string;
  label: string;
  icon: LucideIcon;
  ready: boolean;
  group: NavGroup;
  checkpoint?: number;
  summary?: string;
}

export const NAV: NavItem[] = [
  { to: "/", label: "Home", icon: House, ready: true, group: "Workspace", summary: "Model usage and ads performance at a glance." },
  { to: "/chat", label: "Chat", icon: MessageCircle, ready: true, group: "Workspace", summary: "Ask questions about your ad data." },
  { to: "/ad-accounts", label: "Ad accounts", icon: Building2, ready: true, group: "Ads", summary: "Facebook ad accounts connected through your Meta token." },
  { to: "/insights", label: "Insights", icon: ChartNoAxesColumn, ready: true, group: "Ads", summary: "Spend, results and trends by campaign." },
  { to: "/reports", label: "Reports", icon: FileText, ready: true, group: "Ads", summary: "Build reports, send them now or on a schedule." },
  { to: "/alerts", label: "Alerts", icon: Bell, ready: true, group: "Ads", summary: "Rules are checked after every Meta sync and fire at most once per cooldown." },
  { to: "/approvals", label: "Approvals", icon: ShieldCheck, ready: true, group: "Ads", summary: "The agent only proposes campaign changes. Nothing reaches Meta until you approve it." },
  { to: "/logs", label: "Logs", icon: Rows3, ready: true, group: "System", summary: "Every agent run with its tool calls and model requests." },
  { to: "/settings", label: "Settings", icon: Settings, ready: true, group: "System", summary: "Keys, tokens and limits for models, Meta and notifications." },
];
