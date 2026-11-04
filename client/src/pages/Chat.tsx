import { CircleAlert, LoaderCircle, MessageSquarePlus, MessagesSquare, SendHorizontal, Sparkles, Trash2 } from "lucide-react";
import { Fragment, type FormEvent, useEffect, useRef, useState } from "react";
import { api, type ChatMessage } from "../api";
import { BrandMark } from "../components/BrandMark";
import { ChartView } from "../components/ChartView";
import { EmptyState } from "../components/EmptyState";
import { PageMark } from "../components/PageMark";
import { Tooltip } from "../components/Tooltip";
import { useAsync } from "../hooks";

const PROMPTS = [
  "How much did I spend this week, and how does CPA compare with last week?",
  "Which campaigns have the highest CPA over the last 30 days?",
  "Were there any unusual days in spend recently?",
];

export function Chat() {
  const [conversationId, setConversationId] = useState<string | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [text, setText] = useState("");
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [version, setVersion] = useState(0);
  const conversations = useAsync(() => api.conversations(), [version]);
  const end = useRef<HTMLDivElement>(null);

  useEffect(() => {
    end.current?.scrollIntoView({ block: "end" });
  }, [messages, sending]);

  const open = async (id: string | null) => {
    setConversationId(id);
    setError(null);
    setMessages([]);
    if (!id) return;
    try {
      setMessages((await api.conversation(id)).messages);
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : "Could not load the conversation");
    }
  };

  const remove = async (id: string) => {
    await api.deleteConversation(id);
    if (id === conversationId) await open(null);
    setVersion((value) => value + 1);
  };

  const send = async (event: FormEvent) => {
    event.preventDefault();
    const question = text.trim();
    if (!question || sending) return;
    setText("");
    setError(null);
    setSending(true);
    setMessages((current) => [
      ...current,
      { id: -Date.now(), role: "user", content: question, run_id: null, created_at: new Date().toISOString() },
    ]);
    try {
      const reply = await api.sendMessage(question, conversationId);
      setConversationId(reply.conversation_id);
      setMessages((current) => [...current, reply.message]);
      setVersion((value) => value + 1);
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : "Request failed");
    } finally {
      setSending(false);
    }
  };

  return (
    <div className="chat">
      <section className="chat-thread">
        <div className="chat-messages">
          {messages.length === 0 && !sending && (
            <EmptyState
              mark={<PageMark to="/chat" />}
              title="Ask about your ads"
              text="Heimdall answers from your synced ad data. Try one of these."
              action={
                <div className="chips">
                  {PROMPTS.map((prompt) => (
                    <button key={prompt} type="button" className="chip" onClick={() => setText(prompt)}>
                      <Sparkles size={14} />
                      <span>{prompt}</span>
                    </button>
                  ))}
                </div>
              }
            />
          )}
          {messages.map((message) => (
            <Fragment key={message.id}>
              {(message.content.trim() || !message.charts?.length) && (
                <div className={`bubble ${message.role}`}>
                  {message.role !== "user" && <BrandMark className="bubble-icon" />}
                  <span>{message.content}</span>
                </div>
              )}
              {message.charts?.map((chart, index) => (
                <ChartView key={index} spec={chart} />
              ))}
            </Fragment>
          ))}
          {sending && (
            <div className="bubble assistant muted">
              <LoaderCircle size={16} className="spin" /> <span>Thinking</span>
            </div>
          )}
          {error && (
            <p className="notice error">
              <CircleAlert size={15} className="inline-icon" /> {error}
            </p>
          )}
          <div ref={end} />
        </div>
        <form className="chat-input" onSubmit={send}>
          <textarea
            value={text}
            rows={1}
            placeholder="Ask a question"
            onChange={(event) => setText(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !event.shiftKey) {
                event.preventDefault();
                event.currentTarget.form?.requestSubmit();
              }
            }}
          />
          <Tooltip content="Send (Enter). Shift+Enter for a new line" align="end">
            <button type="submit" className="btn primary" disabled={sending || !text.trim()} aria-label="Send">
              <SendHorizontal size={16} />
            </button>
          </Tooltip>
        </form>
      </section>

      <aside className="chat-list" aria-label="Chat history">
        <button type="button" className="btn" onClick={() => open(null)}>
          <MessageSquarePlus size={16} /> New chat
        </button>
        {conversations.data !== null && conversations.data.length === 0 ? (
          <div className="chat-list-empty">
            <MessagesSquare size={20} />
            <p>No conversations yet</p>
            <small>Ask a question and the chat is saved here, so you can pick it up again later.</small>
          </div>
        ) : (
          <ul>
            {conversations.data?.map((item) => (
              <li key={item.id} className={item.id === conversationId ? "chat-item active" : "chat-item"}>
                <button type="button" className="chat-item-main truncate" onClick={() => open(item.id)}>
                  {item.title}
                </button>
                <Tooltip content="Delete conversation" align="end">
                  <button type="button" className="icon-button" aria-label={`Delete ${item.title}`} onClick={() => remove(item.id)}>
                    <Trash2 size={14} />
                  </button>
                </Tooltip>
              </li>
            ))}
          </ul>
        )}
        <p className="chat-list-note">Chats are saved only on this computer.</p>
      </aside>
    </div>
  );
}
