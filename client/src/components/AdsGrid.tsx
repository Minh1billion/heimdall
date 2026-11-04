import { Eye, MousePointerClick, Target, TrendingUp, Trophy, Wallet } from "lucide-react";
import { Link } from "react-router-dom";
import type { AdsInsights } from "../api";
import { formatDelta, formatMoney, formatNumber, formatPercent, formatRatio } from "../format";
import { metricMeta } from "../alertMeta";
import { Cell } from "./Cell";
import { BarSpark, LineSpark } from "./Sparklines";

interface AdsGridProps {
  data: AdsInsights;
}

export function AdsGrid({ data }: AdsGridProps) {
  const { totals, previous, series, currency } = data;
  const delta = formatDelta(totals.spend, previous.spend);
  const top = data.campaigns.filter((campaign) => campaign.spend > 0).slice(0, 3);

  return (
    <div className="grid">
      <Cell title="Spend" hint={metricMeta("spend").description} icon={Wallet}>
        <div className="value">{formatMoney(totals.spend, currency)}</div>
        <LineSpark values={series.map((point) => point.spend)} color="#e5365b" />
        {delta && <p className="muted small">{delta}</p>}
      </Cell>
      <Cell title="Impressions" hint={metricMeta("impressions").description} icon={Eye}>
        <div className="value">{formatNumber(totals.impressions)}</div>
        <p className="muted small">CPM {formatMoney(totals.cpm, currency)}</p>
      </Cell>
      <Cell title="Clicks" hint={metricMeta("clicks").description} icon={MousePointerClick}>
        <div className="value">{formatNumber(totals.clicks)}</div>
        <BarSpark values={series.map((point) => point.clicks)} color="#0866ff" />
        <p className="muted small">
          CTR {formatPercent(totals.ctr)} · CPC {formatMoney(totals.cpc, currency)}
        </p>
      </Cell>
      <Cell title="Conversions" hint={metricMeta("conversions").description} icon={Target}>
        <div className="value">{formatNumber(Math.round(totals.conversions))}</div>
        <BarSpark values={series.map((point) => point.conversions)} color="#10a37f" />
        <p className="muted small">CPA {formatMoney(totals.cpa, currency)}</p>
      </Cell>
      <Cell title="ROAS" hint={metricMeta("roas").description} icon={TrendingUp}>
        <div className="value">{formatRatio(totals.roas)}</div>
        <p className="muted small">Conversion value divided by spend</p>
      </Cell>
      <Cell
        title="Top campaigns"
        icon={Trophy}
        aside={
          <Link className="link" to="/insights">
            View all
          </Link>
        }
      >
        {top.length ? (
          <ul className="provider-list">
            {top.map((campaign) => (
              <li key={`${campaign.account_id}:${campaign.campaign_id}`}>
                <span className="truncate">{campaign.name}</span>
                <span className="muted mono">{formatMoney(campaign.spend, currency)}</span>
              </li>
            ))}
          </ul>
        ) : (
          <p className="muted small">No spend in this period.</p>
        )}
      </Cell>
    </div>
  );
}
