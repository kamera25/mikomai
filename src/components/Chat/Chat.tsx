import { forwardRef, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronIcon } from "../Icons";
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
    const prevMessagesLength = useRef(messages.length);
    const containerRef = useRef<HTMLDivElement>(null);
    const isUserScrollingUp = useRef(false);
    const isProgrammaticScrolling = useRef(false);
    const [showScrollButton, setShowScrollButton] = useState(false);

    const handleScroll = () => {
      if (!containerRef.current) return;

      if (isProgrammaticScrolling.current) {
        const { scrollTop, scrollHeight, clientHeight } = containerRef.current;
        const isAtBottom = scrollHeight - scrollTop - clientHeight < 80;
        if (isAtBottom) {
          isUserScrollingUp.current = false;
          setShowScrollButton(false);
        }
        return;
      }

      const { scrollTop, scrollHeight, clientHeight } = containerRef.current;
      // Consider it scrolling up if we're not within 80px of the bottom
      const isAtBottom = scrollHeight - scrollTop - clientHeight < 80;
      isUserScrollingUp.current = !isAtBottom;

      // Show button if we are more than 150px away from the bottom
      const isFarFromBottom = scrollHeight - scrollTop - clientHeight > 150;
      setShowScrollButton(isFarFromBottom);
    };

    useEffect(() => {
      if (!containerRef.current) return;

      // Reset scroll block if a new message has been added
      const hasNewMessage = messages.length > prevMessagesLength.current;
      if (hasNewMessage) {
        isUserScrollingUp.current = false;
        setShowScrollButton(false);
      }
      prevMessagesLength.current = messages.length;

      // Scroll to bottom when messages change, unless user is actively scrolling up
      if (!isUserScrollingUp.current && ref && typeof ref !== "function" && ref.current) {
        ref.current.scrollIntoView({ behavior: "auto" });
      }
    }, [messages, ref]);

    const scrollToBottom = () => {
      isUserScrollingUp.current = false;
      setShowScrollButton(false);
      isProgrammaticScrolling.current = true;

      if (ref && typeof ref !== "function" && ref.current) {
        ref.current.scrollIntoView({ behavior: "smooth" });
      }

      setTimeout(() => {
        isProgrammaticScrolling.current = false;
      }, 800);
    };

    return (
      <div className="chat-container">
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
        {showScrollButton && (
          <button className="scroll-to-latest-btn" onClick={scrollToBottom}>
            <ChevronIcon size={14} strokeWidth={2.5} direction="down" />
            <span>最新を見る</span>
          </button>
        )}
      </div>
    );
  }
);

Chat.displayName = "Chat";
