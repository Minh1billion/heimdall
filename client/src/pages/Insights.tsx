import { CalendarDays, ChartNoAxesColumn, Megaphone } from "lucide-react";
import { useState } from "react";
import { api, type AdCampaign, type Range } from "../api";
import { AccountSelect } from "../components/AccountSelect";
import { AdsGrid } from "../components/AdsGrid";
import { EmptyState } from "../components/EmptyState";
import { PageHeader } from "../components/PageHeader";
import { SectionHead } from "../components/SectionHead";
import { StatusBadge } from "../components/StatusBadge";
import { Tooltip } from "../components/Tooltip";
import { metricMeta } from "../alertMeta";
import { RangeToggle } from "../components/RangeToggle";
import { formatMoney, formatNumber, formatPercent, formatRatio } from "../format";
import { useAsync } from "../hooks";
import { campaignStatus, titleCase } from "../statusMeta";

interface Column {
  key: string;
  label: string;
  render: (campaign: AdCampaign, currency: string | null) => string;
}

const COLUMNS: Column[] = [
  { key: "spend", label: "Spend", render: (c, cur) => formatMoney(c.spend, cur) },
  { key: "impressions", label: "Impressions", render: (c) => formatNumber(c.impressions) },
  { key: "clicks", label: "Clicks", render: (c) => formatNumber(c.clicks) },
  { key: "ctr", label: "CTR", render: (c) => formatPercent(c.ctr) },
  { key: "cpc", label: "CPC", render: (c, cur) => formatMoney(c.cpc, cur) },
  { key: "conversions", label: "Conv.", render: (c) => formatNumber(Math.round(c.conversions)) },
  { key: "cpa", label: "CPA", render: (c, cur) => formatMoney(c.cpa, cur) },
  { key: "roas", label: "ROAS", render: (c) => formatRatio(c.roas) },
];

export function Insights() {
  const [range, setRange] = useState<Range>("7d");
  const [accountId, setAccountId] = useState<string | null>(null);
  const accounts = useAsync(() => api.adsAccounts(), []);
  const insights = useAsync(() => api.adsInsights(range, accountId), [range, accountId]);
  const data = insights.data;
  const empty = accounts.data !== null && accounts.data.accounts.length === 0;

  return (
    <div className="page">
      <PageHeader to="/insights" />

      {empty ? (
        <EmptyState
          icon={ChartNoAxesColumn}
          title="No data yet"
          text="Connect and sync an ad account to explore spend, results and trends."
        />
      ) : (
        <>
          <div className="toolbar">
            {data ? (
              <Tooltip content="Compared against the previous period of the same length" align="start">
                <span className="meta-chip" tabIndex={0}>
                  <CalendarDays size={14} />
                  {data.start} to {data.end}
                </span>
              </Tooltip>
            ) : (
              <span />
            )}
            <div className="toolbar-controls">
              <AccountSelect accounts={accounts.data?.accounts ?? []} value={accountId} onChange={setAccountId} />
              <RangeToggle value={range} onChange={setRange} />
            </div>
          </div>

          {insights.error && <p className="notice error">{insights.error}</p>}

          {data && (
            <>
              <AdsGrid data={data} />
              <SectionHead icon={Megaphone} title="Campaigns" text="Includes paused campaigns with no delivery. Hover a column name to see what it measures." />
              <div className="card table-wrap">
                <table className="table campaigns">
                  <thead>
                    <tr>
                      <th>Campaign</th>
                      {COLUMNS.map((column) => (
                        <th key={column.key}>
                          <Tooltip content={`${metricMeta(column.key).label}. ${metricMeta(column.key).description}`} side="bottom" align="end">
                            <span className="th-tip" tabIndex={0}>
                              {column.label}
                            </span>
                          </Tooltip>
                        </th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {data.campaigns.map((campaign) => {
                      const status = campaignStatus(campaign.status);
                      const idle = campaign.spend === 0 && campaign.impressions === 0;
                      const detail = [campaign.objective && titleCase(campaign.objective.replace(/^OUTCOME_/, "")), campaign.campaign_id]
                        .filter(Boolean)
                        .join(" · ");
                      return (
                        <tr key={`${campaign.account_id}:${campaign.campaign_id}`} className={idle ? "idle" : undefined}>
                          <td>
                            <div className="campaign-cell">
                              <span className="truncate campaign-name" title={campaign.name}>
                                {campaign.name}
                              </span>
                              {status && <StatusBadge {...status} />}
                            </div>
                            <span className="truncate campaign-sub mono" title={detail}>
                              {detail}
                            </span>
                          </td>
                          {COLUMNS.map((column) => (
                            <td key={column.key}>{idle ? "-" : column.render(campaign, data.currency)}</td>
                          ))}
                        </tr>
                      );
                    })}
                    {data.campaigns.length === 0 && (
                      <tr>
                        <td colSpan={COLUMNS.length + 1} className="muted">
                          No campaigns yet. Press Sync now on the Ad accounts page to load them.
                        </td>
                      </tr>
                    )}
                  </tbody>
                </table>
              </div>
              <p className="muted small table-note">
                {data.currency === null ? "Mixed currencies, amounts are not converted" : `Amounts in ${data.currency}`}
              </p>
            </>
          )}
        </>
      )}
    </div>
  );
}
