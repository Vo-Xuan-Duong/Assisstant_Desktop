import { FormEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  getRuntimeHealth,
  onAssistantEvent,
  resetConversation,
  restartRuntime,
  submitPrompt,
} from "./api";
import type { AssistantState, ChatMessage, RuntimeHealth } from "./types";

const quickPrompts = [
  "Âm lượng hiện tại bao nhiêu?",
  "Ứng dụng nào đang active?",
  "Máy đang dùng bao nhiêu RAM?",
];

function message(role: ChatMessage["role"], text: string): ChatMessage {
  return { id: crypto.randomUUID(), role, text };
}

export default function App() {
  const [assistantState, setAssistantState] = useState<AssistantState>("idle");
  const [health, setHealth] = useState<RuntimeHealth | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([
    message("system", "Desktop runtime đã sẵn sàng. Antigravity sẽ được khởi tạo khi cần."),
  ]);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const bottomRef = useRef<HTMLDivElement | null>(null);

  const refreshHealth = useCallback(async () => {
    try {
      setHealth(await getRuntimeHealth());
    } catch (error) {
      setHealth({ state: "unhealthy", detail: String(error) });
    }
  }, []);

  useEffect(() => {
    void refreshHealth();
    let disposed = false;
    let unsubscribe: (() => void) | undefined;

    void onAssistantEvent((event) => {
      if (event.type === "state_changed") {
        setAssistantState(event.to);
      }
      if (event.type === "error") {
        setAssistantState("error");
      }
    }).then((unlisten) => {
      if (disposed) unlisten();
      else unsubscribe = unlisten;
    });

    return () => {
      disposed = true;
      unsubscribe?.();
    };
  }, [refreshHealth]);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  const statusLabel = useMemo(() => {
    if (!health) return "Đang kiểm tra runtime";
    if (health.state === "missing") return "Không tìm thấy Antigravity CLI";
    if (health.state === "unhealthy") return "Antigravity CLI có vấn đề";
    if (assistantState === "processing") return "Đang suy luận";
    if (assistantState === "executing") return "Đang thực thi tool";
    if (assistantState === "error") return "Cần khôi phục runtime";
    return "Sẵn sàng";
  }, [assistantState, health]);

  const send = useCallback(
    async (text: string) => {
      const prompt = text.trim();
      if (!prompt || busy) return;

      setMessages((current) => [...current, message("user", prompt)]);
      setInput("");
      setBusy(true);

      try {
        const response = await submitPrompt(prompt);
        setMessages((current) => [...current, message("assistant", response)]);
        await refreshHealth();
      } catch (error) {
        setMessages((current) => [
          ...current,
          message("assistant", `Không thể hoàn thành yêu cầu: ${String(error)}`),
        ]);
        await refreshHealth();
      } finally {
        setBusy(false);
      }
    },
    [busy, refreshHealth],
  );

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    await send(input);
  }

  async function onRestart() {
    setBusy(true);
    try {
      await restartRuntime();
      setAssistantState("idle");
      setMessages((current) => [...current, message("system", "Antigravity runtime đã được khởi động lại.")]);
    } catch (error) {
      setMessages((current) => [...current, message("system", `Restart thất bại: ${String(error)}`)]);
    } finally {
      await refreshHealth();
      setBusy(false);
    }
  }

  async function onNewConversation() {
    setBusy(true);
    try {
      await resetConversation();
      setAssistantState("idle");
      setMessages([message("system", "Đã tạo phiên hội thoại mới.")]);
    } finally {
      await refreshHealth();
      setBusy(false);
    }
  }

  return (
    <main className="app-shell">
      <header className="topbar">
        <div>
          <p className="eyebrow">ASSISSTANT DESKTOP</p>
          <h1>Antigravity Assistant</h1>
        </div>
        <div className={`runtime-pill runtime-${health?.state ?? "checking"}`}>
          <span className="status-dot" />
          <span>{statusLabel}</span>
        </div>
      </header>

      <section className="runtime-card">
        <div>
          <span className="section-label">Runtime</span>
          <strong>{health?.detail || "Antigravity CLI"}</strong>
          <small>{health?.conversation_id ? `Conversation ${health.conversation_id}` : "Chưa có conversation active"}</small>
        </div>
        <div className="runtime-actions">
          <button type="button" className="secondary" disabled={busy} onClick={() => void refreshHealth()}>
            Kiểm tra
          </button>
          <button type="button" className="secondary" disabled={busy} onClick={() => void onRestart()}>
            Restart
          </button>
          <button type="button" className="secondary" disabled={busy} onClick={() => void onNewConversation()}>
            Phiên mới
          </button>
        </div>
      </section>

      <section className="conversation" aria-live="polite">
        {messages.map((item) => (
          <article key={item.id} className={`message message-${item.role}`}>
            <span>{item.role === "user" ? "Bạn" : item.role === "assistant" ? "Assistant" : "System"}</span>
            <p>{item.text}</p>
          </article>
        ))}
        {busy && (
          <article className="message message-assistant pending">
            <span>Assistant</span>
            <p>Đang xử lý<span className="thinking-dots">...</span></p>
          </article>
        )}
        <div ref={bottomRef} />
      </section>

      <section className="quick-prompts">
        {quickPrompts.map((prompt) => (
          <button key={prompt} type="button" disabled={busy} onClick={() => void send(prompt)}>
            {prompt}
          </button>
        ))}
      </section>

      <form className="composer" onSubmit={onSubmit}>
        <textarea
          value={input}
          onChange={(event) => setInput(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault();
              void send(input);
            }
          }}
          placeholder="Nhập yêu cầu cho Assistant..."
          rows={2}
          disabled={busy}
        />
        <button type="submit" className="primary" disabled={busy || !input.trim()}>
          Gửi
        </button>
      </form>

      <footer>
        <span>Alt + Space để mở Assistant</span>
        <span>Voice sẽ được thêm sau khi Text MVP ổn định</span>
      </footer>
    </main>
  );
}
