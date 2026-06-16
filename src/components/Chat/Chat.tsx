import { forwardRef, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { Message } from "../../types";
import { TimelineEvent } from "./TimelineEvent";
import "./Chat.css";

interface ChatProps {
  messages: Message[];
  formatMessageTime: (isoString?: string) => string;
}

export const Chat = forwardRef<HTMLDivElement, ChatProps>(
  ({ messages, formatMessageTime }, ref) => {
    const { t } = useTranslation();
    const containerRef = useRef<HTMLDivElement>(null);
    const isUserScrollingUp = useRef(false);

    const handleScroll = () => {
      if (!containerRef.current) return;
      const { scrollTop, scrollHeight, clientHeight } = containerRef.current;
      // Consider it scrolling up if we're not within 50px of the bottom
      isUserScrollingUp.current = scrollHeight - scrollTop - clientHeight > 50;
    };

    useEffect(() => {
      // Scroll to bottom when messages change, unless user is actively scrolling up
      if (!isUserScrollingUp.current && ref && typeof ref !== "function" && ref.current) {
        ref.current.scrollIntoView({ behavior: "smooth" });
      }
    }, [messages, ref]);

    return (
      <div className="chat-history" ref={containerRef} onScroll={handleScroll}>
        {messages.length === 0 ? (
          <div className="empty-state">
            <div>
              <img width="64px" height="64px" src="/mikomai.png" alt="mikomai" />
            </div>
            <h3>mikomai</h3>
            <p>
              {t("chat.welcome_message_1")}
            </p>
            <p>
              {t("chat.welcome_message_2")}
            </p>
          </div>
        ) : (
          messages.map((msg, idx) => (
            <TimelineEvent
              key={msg.task_id || idx}
              msg={msg}
              formatMessageTime={formatMessageTime}
            />
          ))
        )}
        <div ref={ref} />
      </div>
    );
  }
);

Chat.displayName = "Chat";
