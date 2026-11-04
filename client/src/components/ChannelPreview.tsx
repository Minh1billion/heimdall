import { LoaderCircle, TriangleAlert } from "lucide-react";
import { Fragment, type ReactNode } from "react";
import type { EmailContext, Rendered, RenderedSet } from "../api";
import { BrandMark } from "./BrandMark";

type ChannelName = "telegram" | "slack" | "zalo" | "email";

interface ChannelPreviewProps {
  channel: string;
  rendered: RenderedSet | null;
  email: EmailContext | null;
  loading?: boolean;
  emptyText: string;
  note?: ReactNode;
  actions?: ReactNode;
}

function nowLabel(): string {
  return new Date().toLocaleTimeString("en-US", { hour: "numeric", minute: "2-digit" });
}

function slackText(text: string): ReactNode {
  return text.split("\n").map((line, index) => (
    <Fragment key={index}>
      {index > 0 && <br />}
      {line.split(/(\*[^*\n]+\*)/g).map((part, i) =>
        /^\*[^*\n]+\*$/.test(part) ? <strong key={i}>{part.slice(1, -1)}</strong> : <Fragment key={i}>{part}</Fragment>,
      )}
    </Fragment>
  ));
}

function ChatFrame({ message, kind }: { message: Rendered; kind: "telegram" | "zalo" }) {
  return (
    <div className={`pv pv-${kind}`}>
      <div className="pv-bar">
        <BrandMark className="pv-avatar" />
        <div>
          <strong>Heimdall</strong>
          <span>bot</span>
        </div>
      </div>
      <div className="pv-chat">
        <div className="pv-bubble">
          <p>{message.body}</p>
          <time>{nowLabel()}</time>
        </div>
      </div>
    </div>
  );
}

function SlackFrame({ message }: { message: Rendered }) {
  return (
    <div className="pv pv-slack">
      <div className="pv-row">
        <BrandMark className="pv-avatar" />
        <div className="pv-col">
          <div className="pv-who">
            <strong>Heimdall</strong>
            <span className="pv-tag">APP</span>
            <time>{nowLabel()}</time>
          </div>
          <p>{slackText(message.body)}</p>
        </div>
      </div>
    </div>
  );
}

function EmailFrame({ message, context }: { message: Rendered; context: EmailContext | null }) {
  const from = context?.from;
  const to = context?.to ?? [];
  return (
    <div className="pv pv-email">
      <div className="pv-mail-head">
        <h4>{message.subject}</h4>
        <dl>
          <dt>From</dt>
          <dd>{from ? `Heimdall <${from}>` : <span className="pv-missing">Sender not set in Settings</span>}</dd>
          <dt>To</dt>
          <dd>{to.length ? to.join(", ") : <span className="pv-missing">No recipients set in Settings</span>}</dd>
          <dt>Date</dt>
          <dd>{new Date().toLocaleString("en-US", { dateStyle: "medium", timeStyle: "short" })}</dd>
        </dl>
      </div>
      <pre className="pv-mail-body">{message.body}</pre>
    </div>
  );
}

const NOTES: Record<ChannelName, string> = {
  telegram: "Telegram receives plain text, so nothing is formatted.",
  slack: "Slack shows the title in bold. The rest is plain text.",
  zalo: "Zalo receives plain text, so nothing is formatted.",
  email: "Sent as a plain-text email.",
};

export function ChannelPreview({ channel, rendered, email, loading = false, emptyText, note, actions }: ChannelPreviewProps) {
  const active: ChannelName = channel === "slack" || channel === "email" || channel === "zalo" ? channel : "telegram";
  const message = rendered?.[active] ?? null;

  return (
    <div className="preview-card card">
      {actions && <div className="preview-head">{actions}</div>}

      {note && <p className="preview-note">{note}</p>}

      <div className={loading && message ? "preview-stage busy" : "preview-stage"}>
        {message ? (
          <>
            {(active === "telegram" || active === "zalo") && <ChatFrame message={message} kind={active} />}
            {active === "slack" && <SlackFrame message={message} />}
            {active === "email" && <EmailFrame message={message} context={email} />}
          </>
        ) : (
          <div className="pv-empty">
            {loading ? <LoaderCircle size={18} className="spin" /> : null}
            <p>{loading ? "Building the preview…" : emptyText}</p>
          </div>
        )}
      </div>

      <div className="preview-foot">
        <span>{NOTES[active]}</span>
        {message?.limit !== null && message?.limit !== undefined && (
          <span className={message.clipped ? "pv-count over" : "pv-count"}>
            {[...message.body].length.toLocaleString("en-US")} / {message.limit.toLocaleString("en-US")}
          </span>
        )}
      </div>
      {message?.clipped && (
        <p className="inline-warn">
          <TriangleAlert size={15} />
          Longer than the channel allows, so the end is cut off when sent.
        </p>
      )}
    </div>
  );
}
