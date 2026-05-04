import { forwardRef } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import remarkMath from 'remark-math';
import rehypeKatex from 'rehype-katex';
import { Terminal } from '../Terminal';
import { Message } from '../../types';

interface ChatProps {
  messages: Message[];
  formatMessageTime: (isoString?: string) => string;
}

export const Chat = forwardRef<HTMLDivElement, ChatProps>(({ messages, formatMessageTime }, ref) => {
  return (
    <div className="chat-history">
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
          <div key={idx} className={`message-container ${msg.role}`}>
            {msg.role === 'user' && (
              <div className="message-header">
                <div className="header-line"></div>
                <span className="message-time">{formatMessageTime(msg.timestamp)}</span>
              </div>
            )}
            <div className={`message ${msg.role}`}>
              <div className="message-bubble markdown-body">
                {!!msg.isToolLoading ? (
                  <div className="tool-status-container" data-is-loading="true">
                    <div className="status-spinner"></div>
                    <span>{msg.content}</span>
                  </div>
                ) : (
                  msg.content.split(/(```[\s\S]*?```)/).map((part, i) => {
                    if (part.startsWith("```")) {
                      const isTerminal = part.startsWith("```terminal");
                      const content = part.replace(/```(\w+)?\n?/, "").replace(/```$/, "");

                      if (isTerminal) {
                        return <Terminal key={i} content={content} />;
                      }

                      return <pre key={i} className="code-block"><code>{content}</code></pre>;
                    }
                    return (
                      <ReactMarkdown
                        key={i}
                        remarkPlugins={[remarkGfm, remarkMath]}
                        rehypePlugins={[rehypeKatex]}
                      >
                        {part}
                      </ReactMarkdown>
                    );
                  })
                )}
              </div>
            </div>
          </div>
        ))
      )}
      <div ref={ref} />
    </div>
  );
});

Chat.displayName = 'Chat';
