import { Building2, Clock, Coins, ExternalLink, Globe, KeyRound, LoaderCircle, RefreshCw, TriangleAlert } from "lucide-react";
import { useState } from "react";
import { Link } from "react-router-dom";
import { api } from "../api";
import { EmptyState } from "../components/EmptyState";
import { BrandIcon } from "../components/IconTile";
import { PageHeader } from "../components/PageHeader";
import { StatusBadge } from "../components/StatusBadge";
import { Tooltip } from "../components/Tooltip";
import { When } from "../components/When";
import { useAsync } from "../hooks";
import { accountStatus } from "../statusMeta";

const TOKEN_REFS = [
  {
    title: "System user access tokens",
    text: "Create a system user in Business Manager and generate a token that does not expire. Best for Heimdall.",
    href: "https://developers.facebook.com/docs/marketing-api/system-users/create-retrieve-update",
  },
  {
    title: "Business Manager system users",
    text: "Open Business settings, add a system user, assign ad accounts, then press Generate token.",
    href: "https://business.facebook.com/settings/system-users",
  },
  {
    title: "Marketing API authorization",
    text: "Which permissions the token needs: ads_read to sync, ads_management to apply approved changes.",
    href: "https://developers.facebook.com/docs/marketing-api/overview/authorization",
  },
  {
    title: "Graph API Explorer",
    text: "Quick way to get a short-lived user token for a first test. It expires within hours.",
    href: "https://developers.facebook.com/tools/explorer/",
  },
  {
    title: "Access token guide",
    text: "Token types, lifetimes and how to exchange a short-lived token for a long-lived one.",
    href: "https://developers.facebook.com/docs/facebook-login/guides/access-tokens",
  },
] as const;

function TokenRefs() {
  return (
    <section className="refs" aria-labelledby="token-refs-title">
      <div className="refs-head">
        <KeyRound size={16} />
        <h3 id="token-refs-title">How to get a Meta access token</h3>
        <span className="grow" />
        <Link className="link" to="/settings">
          Add it in Settings
        </Link>
      </div>
      <ul className="refs-list">
        {TOKEN_REFS.map((ref) => (
          <li key={ref.href}>
            <a className="ref-link" href={ref.href} target="_blank" rel="noreferrer noopener">
              <span className="ref-text">
                <span className="ref-title">{ref.title}</span>
                <span className="ref-sub">{ref.text}</span>
              </span>
              <ExternalLink size={14} />
            </a>
          </li>
        ))}
      </ul>
    </section>
  );
}

export function AdAccounts() {
  const [version, setVersion] = useState(0);
  const [syncing, setSyncing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const accounts = useAsync(() => api.adsAccounts(), [version]);
  const data = accounts.data;

  const sync = async () => {
    setSyncing(true);
    setError(null);
    try {
      await api.adsSync();
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : "Sync failed");
    } finally {
      setSyncing(false);
      setVersion((value) => value + 1);
    }
  };

  return (
    <div className="page">
      <PageHeader
        to="/ad-accounts"
        actions={
          data?.configured && (
            <Tooltip content="Fetch the latest accounts and insights from Meta" align="end">
              <button type="button" className="btn primary" disabled={syncing} onClick={sync}>
                {syncing ? <LoaderCircle size={16} className="spin" /> : <RefreshCw size={16} />}
                {syncing ? "Syncing" : "Sync now"}
              </button>
            </Tooltip>
          )
        }
      />

      {accounts.error && <p className="notice error">{accounts.error}</p>}
      {error && <p className="notice error">{error}</p>}

      {data && !data.configured && (
        <EmptyState
          icon={Building2}
          title="Meta is not connected"
          text="Add at least one Meta access token with ads_read access in Settings. It takes effect right away, no restart needed."
        />
      )}

      {data && !data.configured && <TokenRefs />}

      {data?.configured && data.accounts.length === 0 && (
        <EmptyState
          icon={Building2}
          title="No ad accounts yet"
          text="Press Sync now to load the ad accounts your tokens can access."
        />
      )}

      {data?.configured && data.accounts.length === 0 && <TokenRefs />}

      {data && data.accounts.length > 0 && (
        <ul className="card-grid">
          {data.accounts.map((account) => (
            <li key={account.id} className="item">
              <div className="item-head">
                <BrandIcon name="meta" size="md" />
                <div className="item-title">
                  <h3 className="truncate" title={account.name}>
                    {account.name}
                  </h3>
                  <p className="item-sentence mono truncate">{account.id}</p>
                </div>
                <StatusBadge {...accountStatus(account.status)} />
              </div>

              <div className="meta-chips">
                <Tooltip
                  content={
                    account.connection_name
                      ? "The token that syncs this account"
                      : "No token covers this account, so it cannot sync or be changed. Add a token that can access it in Settings"
                  }
                  align="start"
                >
                  <span className={account.connection_name ? "meta-chip" : "meta-chip warn"} tabIndex={0}>
                    <KeyRound size={14} />
                    <span className="truncate">{account.connection_name ?? "No token"}</span>
                  </span>
                </Tooltip>
                <Tooltip content="Currency amounts are reported in" align="start">
                  <span className="meta-chip" tabIndex={0}>
                    <Coins size={14} />
                    {account.currency || "-"}
                  </span>
                </Tooltip>
                <Tooltip content="Account timezone. Daily numbers follow this clock" align="end">
                  <span className="meta-chip" tabIndex={0}>
                    <Globe size={14} />
                    <span className="truncate">{account.timezone}</span>
                  </span>
                </Tooltip>
              </div>

              <div className="item-foot">
                <span className="foot-item">
                  <Clock size={14} />
                  Synced <When value={account.last_synced_at} align="start" />
                </span>
              </div>

              {account.last_error && (
                <div className="item-error" role="alert">
                  <TriangleAlert size={15} />
                  <span>{account.last_error}</span>
                </div>
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
