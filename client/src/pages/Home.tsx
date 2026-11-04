import { Building2, ChartNoAxesColumn, Coins, DatabaseZap, Plug, CircleCheck, Timer, Zap } from "lucide-react";
import { useState } from "react";
import { Link } from "react-router-dom";
import { api, type Range } from "../api";
import { AdsGrid } from "../components/AdsGrid";
import { Banner } from "../components/Banner";
import { BrandIcon } from "../components/IconTile";
import { Cell } from "../components/Cell";
import { PageHeader } from "../components/PageHeader";
import { EmptyState } from "../components/EmptyState";
import { RangeToggle } from "../components/RangeToggle";
import { SectionHead } from "../components/SectionHead";
import { BarSpark, LineSpark } from "../components/Sparklines";
import { formatMs, formatNumber, formatPercent } from "../format";
import { useAsync, useStoredFlag } from "../hooks";

export function Home() {
  const [range, setRange] = useState<Range>("7d");
  const [dismissed, setDismissed] = useStoredFlag("heimdall.banner.dismissed");
  const usage = useAsync(() => api.llmDashboard(range), [range]);
  const providers = useAsync(() => api.providers(), []);
  const accounts = useAsync(() => api.adsAccounts(), []);
  const ads = useAsync(() => api.adsInsights(range, null), [range]);
  const data = usage.data;
  const tokenSeries = data?.series.map((point) => point.tokens) ?? [];
  const requestSeries = data?.series.map((point) => point.requests) ?? [];
  const successPercent = data?.success_rate != null ? data.success_rate * 100 : 0;

  return (
    <div className="page">
      <PageHeader to="/" />

      {!dismissed && <Banner onDismiss={() => setDismissed(true)} />}

      <div className="toolbar">
        <p className="toolbar-note muted small">Showing the last {range === "24h" ? "24 hours" : range.replace("d", " days")}</p>
        <RangeToggle value={range} onChange={setRange} />
      </div>

      {usage.error && <p className="notice error">{usage.error}</p>}

      <div className="grid">
        <Cell title="Total tokens" icon={Coins} hint="Input plus output tokens across all model requests in this period.">
          <div className="value">{formatNumber(data?.total_tokens)}</div>
          <LineSpark values={tokenSeries} color="#e5365b" />
        </Cell>
        <Cell title="Model requests" icon={Zap} hint="Calls made to your language model providers.">
          <div className="value">{formatNumber(data?.total_requests)}</div>
          <BarSpark values={requestSeries} color="#10a37f" />
        </Cell>
        <Cell
          title="Prompt caching: hit rate"
          icon={DatabaseZap}
          hint="Share of input tokens served from the provider cache. Higher means faster and cheaper requests."
        >
          <div className="value">{formatPercent(data?.cache_hit_rate)}</div>
          <div className="rule" />
        </Cell>
        <Cell
          title="Providers"
          icon={Plug}
          aside={
            <Link className="link" to="/settings">
              Manage
            </Link>
          }
        >
          {providers.data && providers.data.some((provider) => provider.configured) ? (
            <ul className="provider-list">
              {providers.data
                .filter((provider) => provider.configured)
                .map((provider) => (
                  <li key={provider.name}>
                    <BrandIcon name={provider.name} size="sm" />
                    <span>{provider.label}</span>
                    <span className="muted mono">{provider.model}</span>
                  </li>
                ))}
            </ul>
          ) : (
            <p className="muted small">
              No provider connected yet. <Link to="/settings">Add an API key</Link> to get started.
            </p>
          )}
        </Cell>
        <Cell title="Success rate" icon={CircleCheck} hint="Share of model requests that completed without an error.">
          <div className="value">{formatPercent(data?.success_rate)}</div>
          <div className="meter">
            <div className="meter-fill" style={{ width: `${successPercent}%` }} />
          </div>
        </Cell>
        <Cell title="Average latency" icon={Timer} hint="Average time a model takes to respond.">
          <div className="value">{formatMs(data?.avg_latency_ms)}</div>
          <p className="muted small">Across successful model requests</p>
        </Cell>
      </div>

      <SectionHead icon={ChartNoAxesColumn} title="Ads performance" text="Across all connected ad accounts, for the period selected above." />
      {ads.error && <p className="notice error">{ads.error}</p>}
      {accounts.data && accounts.data.accounts.length === 0 ? (
        <EmptyState
          icon={Building2}
          title="No ad accounts connected"
          text="Connect a Facebook ad account to see spend, results and trends here."
          action={
            <Link className="btn primary" to="/ad-accounts">
              Connect ad account
            </Link>
          }
        />
      ) : (
        ads.data && <AdsGrid data={ads.data} />
      )}
    </div>
  );
}
