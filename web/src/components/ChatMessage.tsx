import React, { useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import rehypeHighlight from 'rehype-highlight';
import rehypeKatex from 'rehype-katex';
import type { OpenZMessage } from '../types';
import { ToolExecutionCard } from './ToolExecutionCard';
import { SecurityGuardPrompt } from './SecurityGuardPrompt';
import { useOpenZStore } from '../store/useOpenZStore';
import { User, Copy, Check, Brain, ChevronDown, ChevronRight, Info } from 'lucide-react';
import 'katex/dist/katex.min.css';
import 'highlight.js/styles/github-dark.css';

interface ChatMessageProps {
  message: OpenZMessage;
}

// Helper Component for Markdown Code Block to manage individual copy states and animations
const MarkdownCodeBlock: React.FC<{
  language?: string;
  children: string;
  className?: string;
  [key: string]: any;
}> = ({ language, children, className, ...props }) => {
  const [copied, setCopied] = useState(false);

  const handleCopy = () => {
    navigator.clipboard.writeText(String(children).replace(/\n$/, ''));
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="relative my-3 rounded-lg bg-zinc-950 font-mono text-xs overflow-hidden border border-zinc-800/80">
      <div className="flex items-center justify-between border-b border-zinc-800/80 bg-zinc-900/90 px-3 py-1.5 text-[10px] text-zinc-400 select-none">
        <span className="font-semibold text-amber-500/80">{language || 'code'}</span>
        <button
          onClick={handleCopy}
          className="flex items-center gap-1 text-zinc-400 hover:text-zinc-100 transition duration-150 active:scale-95"
        >
          {copied ? (
            <span className="flex items-center gap-1 text-emerald-400 font-semibold animate-in fade-in zoom-in-95 duration-200">
              <Check className="h-3 w-3" />
              <span>Copied!</span>
            </span>
          ) : (
            <span className="flex items-center gap-1 transition-all duration-150">
              <Copy className="h-3 w-3" />
              <span>Copy code</span>
            </span>
          )}
        </button>
      </div>
      <pre className="p-3 overflow-x-auto text-[12px] text-zinc-200">
        <code className={className} {...props}>
          {children}
        </code>
      </pre>
    </div>
  );
};

export const ChatMessage: React.FC<ChatMessageProps> = ({ message }) => {
  const handleSecurityChoice = useOpenZStore((s) => s.handleSecurityChoice);
  const [copied, setCopied] = useState(false);
  const [showTrace, setShowTrace] = useState(false);

  const isUser = message.role === 'user';

  const copyContent = () => {
    navigator.clipboard.writeText(message.content);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  // Muted system/tool/notice messages: no avatar, no bubble.
  if (message.isNotice || message.role === 'system' || message.role === 'tool') {
    return (
      <div className="flex w-full justify-center py-2">
        <div className="flex max-w-2xl items-center gap-2 rounded-lg border border-border/40 bg-muted/30 px-3 py-2 text-xs text-muted-foreground">
          <Info className="h-3.5 w-3.5 shrink-0 text-amber-500" />
          <span className="min-w-0 break-words">{message.content}</span>
        </div>
      </div>
    );
  }

  return (
    <div className={`group relative flex w-full gap-3 py-4 text-sm ${isUser ? 'justify-end' : 'justify-start'}`}>
      {!isUser && (
        <div className="flex h-8 w-8 shrink-0 select-none items-center justify-center text-xl leading-none">
          <span className="bg-gradient-to-r from-amber-500 to-orange-500 bg-clip-text text-transparent">
            🦊
          </span>
        </div>
      )}

      <div className={`flex w-fit max-w-3xl min-w-0 flex-col gap-1.5 ${isUser ? 'items-end' : 'items-start'}`}>
        {/* Role & Model Tag */}
        <div className="flex items-center gap-2 text-[11px] font-medium text-muted-foreground select-none">
          <span>{isUser ? 'You' : 'OpenZ Agent'}</span>
          {message.model && <span className="rounded bg-muted px-1.5 py-0.5 text-[10px]">{message.model}</span>}
          <span className="text-[10px] opacity-70">
            {new Date(message.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
          </span>
        </div>

        {/* Message Bubble Container */}
        <div
          className={`relative w-fit max-w-full min-w-0 rounded-2xl px-4 py-3 leading-relaxed shadow-sm transition-all ${
            isUser
              ? 'bg-primary text-primary-foreground rounded-tr-xs'
              : 'bg-card/70 border border-border/50 text-foreground rounded-tl-xs backdrop-blur-xs'
          }`}
        >
          {/* Loading / Initial thinking state */}
          {!message.content && !message.reasoningContent && message.isStreaming && (
            <div className="flex items-center gap-2 text-muted-foreground select-none py-1">
              <span className="relative flex h-2 w-2">
                <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-amber-500 opacity-75"></span>
                <span className="relative inline-flex rounded-full h-2 w-2 bg-amber-600"></span>
              </span>
              <span className="text-xs font-semibold animate-pulse text-amber-500/80">Thinking...</span>
            </div>
          )}

          {/* Thinking trace collapsible (streamed reasoning or <think> tags) */}
          {message.reasoningContent && (
            <div className="mb-3 rounded-lg border border-amber-500/30 bg-amber-500/5 p-2.5 text-xs text-amber-200/90">
              <button
                onClick={() => setShowTrace(!showTrace)}
                className="flex items-center gap-1.5 font-medium text-amber-400 select-none hover:underline"
              >
                <Brain className="h-3.5 w-3.5" />
                <span>Thinking Process</span>
                {showTrace ? <ChevronDown className="h-3 w-3" /> : <ChevronRight className="h-3 w-3" />}
              </button>
              {showTrace && (
                <div className="mt-2 max-h-64 overflow-y-auto font-mono text-[11px] whitespace-pre-wrap opacity-90">
                  {message.reasoningContent}
                </div>
              )}
            </div>
          )}

          {/* Tool executions attached to this message */}
          {message.toolCalls && message.toolCalls.length > 0 && (
            <div className="my-1 w-full min-w-0">
              {message.toolCalls.map((tool) => (
                <ToolExecutionCard key={tool.id} tool={tool} />
              ))}
            </div>
          )}

          {/* Security interception prompts (plural) */}
          {message.securityPrompts && message.securityPrompts.length > 0 && (
            <div className="my-1 space-y-1 w-full min-w-0">
              {message.securityPrompts.map((prompt) => (
                <SecurityGuardPrompt
                  key={prompt.id}
                  prompt={prompt}
                  onChoice={(choice) => handleSecurityChoice(prompt.id, choice)}
                />
              ))}
            </div>
          )}

          {/* Main Markdown Output */}
          {message.content && (
            <div className="prose dark:prose-invert prose-xs max-w-none break-words">
              <ReactMarkdown
                remarkPlugins={[remarkGfm]}
                rehypePlugins={[rehypeHighlight, rehypeKatex]}
                components={{
                  code({ node, inline, className, children, ...props }: any) {
                    const match = /language-(\w+)/.exec(className || '');
                    return !inline ? (
                      <MarkdownCodeBlock
                        className={className}
                        language={match ? match[1] : undefined}
                        {...props}
                      >
                        {String(children)}
                      </MarkdownCodeBlock>
                    ) : (
                      <code className="rounded bg-muted px-1.5 py-0.5 font-mono text-xs font-semibold text-amber-500 dark:text-amber-400" {...props}>
                        {children}
                      </code>
                    );
                  },
                  table({ children }: any) {
                    return (
                      <div className="my-4 overflow-x-auto rounded-xl border border-border/80 bg-card/40 shadow-sm w-full">
                        <table className="w-full border-collapse text-left text-xs text-foreground/90 min-w-[500px]">
                          {children}
                        </table>
                      </div>
                    );
                  },
                  thead({ children }: any) {
                    return <thead className="border-b border-border bg-muted/40 text-[10px] font-bold uppercase tracking-wider text-muted-foreground">{children}</thead>;
                  },
                  tbody({ children }: any) {
                    return <tbody className="divide-y divide-border/60">{children}</tbody>;
                  },
                  tr({ children }: any) {
                    return <tr className="hover:bg-muted/20 transition-colors">{children}</tr>;
                  },
                  th({ children }: any) {
                    return <th className="px-4 py-3 font-semibold select-none">{children}</th>;
                  },
                  td({ children }: any) {
                    return <td className="px-4 py-3 align-middle break-words">{children}</td>;
                  },
                }}
              >
                {message.content}
              </ReactMarkdown>
            </div>
          )}

          {/* Streaming Cursor */}
          {message.isStreaming && (
            <span className="ml-1 inline-block h-4 w-1.5 animate-pulse rounded bg-amber-500 align-middle" />
          )}
        </div>

        {/* Action buttons under the bubble (visible on hover) */}
        {!message.isStreaming && (
          <div className="flex items-center gap-2 mt-0.5 opacity-0 group-hover:opacity-100 transition-opacity select-none px-2">
            <button
              onClick={copyContent}
              className="flex items-center gap-1.5 text-[11px] text-muted-foreground hover:text-foreground transition"
              title="Copy message"
            >
              {copied ? (
                <>
                  <Check className="h-3 w-3 text-emerald-500" />
                  <span className="text-emerald-500 font-semibold">Copied</span>
                </>
              ) : (
                <>
                  <Copy className="h-3 w-3" />
                  <span>Copy</span>
                </>
              )}
            </button>
          </div>
        )}
      </div>

      {isUser && (
        <div className="flex h-8 w-8 shrink-0 select-none items-center justify-center rounded-full bg-secondary text-secondary-foreground border border-border">
          <User className="h-4 w-4" />
        </div>
      )}
    </div>
  );
};
