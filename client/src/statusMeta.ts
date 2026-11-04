import { Ban, CircleCheck, CircleX, Clock, Coins, CirclePause, Hourglass, TriangleAlert, type LucideIcon } from "lucide-react";
import type { ApprovalStatus } from "./api";
import type { StatusInfo } from "./components/StatusBadge";

export function titleCase(value: string): string {
  const text = value.replace(/[_-]+/g, " ").trim().toLowerCase();
  return text.charAt(0).toUpperCase() + text.slice(1);
}

export function approvalStatus(status: ApprovalStatus): StatusInfo {
  switch (status) {
    case "pending":
      return { tone: "warn", label: "Waiting for you", icon: Clock, tip: "Nothing changes on Meta until you approve." };
    case "executed":
      return { tone: "ok", label: "Applied", icon: CircleCheck, tip: "The change was applied on Meta." };
    case "failed":
      return { tone: "fail", label: "Failed", icon: TriangleAlert, tip: "Approved, but Meta could not apply it. See the result below." };
    case "rejected":
      return { tone: "neutral", label: "Rejected", icon: Ban, tip: "You rejected this request. Nothing was changed." };
    case "expired":
      return { tone: "neutral", label: "Expired", icon: Hourglass, tip: "No decision was made in time, so it was discarded." };
  }
}

export function accountStatus(status: string): StatusInfo {
  const key = status.toLowerCase();
  if (key === "active") return { tone: "ok", label: "Active", icon: CircleCheck, tip: "The account can run ads." };
  if (["disabled", "closed", "permanently_closed"].includes(key)) {
    return { tone: "fail", label: titleCase(key), icon: CircleX, tip: "Meta reports this account cannot run ads." };
  }
  if (key.includes("pending") || key.includes("review") || key.includes("unsettled") || key.includes("grace")) {
    return { tone: "warn", label: titleCase(key), icon: TriangleAlert, tip: "Meta needs something resolved on this account (review or payment)." };
  }
  return { tone: "neutral", label: titleCase(key) || "Unknown", tip: `Status reported by Meta: ${status}` };
}

export function campaignStatus(status: string | null): StatusInfo | null {
  if (!status) return null;
  const key = status.toLowerCase();
  if (key === "active") return { tone: "ok", label: "Active", icon: CircleCheck, tip: "Delivering ads now." };
  if (key === "paused" || key === "campaign_paused") {
    return { tone: "neutral", label: "Paused", icon: CirclePause, tip: "Paused, so it is not delivering or spending." };
  }
  if (key === "with_issues" || key === "disapproved" || key === "pending_review") {
    return { tone: "warn", label: titleCase(key), icon: TriangleAlert, tip: "Meta needs something resolved before this can deliver." };
  }
  if (key === "deleted" || key === "archived") return { tone: "neutral", label: titleCase(key), tip: "No longer in use on Meta." };
  return { tone: "neutral", label: titleCase(key), tip: `Status reported by Meta: ${status}` };
}

export function runStatus(status: string): StatusInfo {
  if (status === "ok") return { tone: "ok", label: "Success", icon: CircleCheck, tip: "The agent finished and replied." };
  return { tone: "fail", label: titleCase(status) || "Failed", icon: TriangleAlert, tip: "The run stopped with an error. Open it for details." };
}

export function toolLabel(tool: string): { label: string; icon: LucideIcon; tone: "amber" | "blue"; tip: string } {
  if (tool === "propose_pause_campaign") {
    return { label: "Pause campaign", icon: CirclePause, tone: "amber", tip: "The agent suggests pausing this campaign." };
  }
  const label = titleCase(tool.replace(/^propose_/, ""));
  return { label, icon: Coins, tone: "blue", tip: "The agent suggests this change to the campaign." };
}
