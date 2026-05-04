import { forwardRef, useEffect, useRef } from 'react';
import { Message } from '../../types';
import { TimelineEvent } from './TimelineEvent';

interface ChatProps {
  messages: Message[];
  formatMessageTime: (isoString?: string) => string;
}

export const Chat = forwardRef<HTMLDivElement, ChatProps>(({ messages, formatMessageTime }, ref) => {
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
    if (!isUserScrollingUp.current && ref && typeof ref !== 'function' && ref.current) {
      ref.current.scrollIntoView({ behavior: 'smooth' });
    }
  }, [messages, ref]);

  return (
    <div className="chat-history" ref={containerRef} onScroll={handleScroll}>
      {messages.length === 0 ? (
        <div className="empty-state">
          <div>
            <img width="64px" height="64px" src="/public/mikomai.png" alt="mikomai" />
          </div>
          <h3>mikomai</h3>
          <p>ネットワーク構築やトラブルシュートをサポートします。サポートして欲しいことを伝えてみてください。</p>
          <p>例えば、マニュアルの取得、スイッチの状態確認、構成変更の提案などご支援いたします。</p>
        </div>
      ) : (
        messages.map((msg, idx) => (
          <TimelineEvent key={msg.task_id || idx} msg={msg} formatMessageTime={formatMessageTime} />
        ))
      )}
      <div ref={ref} />
    </div>
  );
});

Chat.displayName = 'Chat';
