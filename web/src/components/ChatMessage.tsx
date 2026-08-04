import React, { useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import rehypeHighlight from 'rehype-highlight';
import rehypeKatex from 'rehype-katex';
import type { OpenZMessage } from '../types';
import { ToolExecutionCard } from './ToolExecutionCard';
import { SecurityGuardPrompt } from './SecurityGuardPrompt';
import { useOpenZStore } from '../store/useOpenZStore';
import { User, Copy, Check, Sparkles, Brain, ChevronDown, ChevronRight, Info } from 'lucide-react';
import 'katex/dist/katex.min.css';
import 'highlight.js/styles/github-dark.css';

interface ChatMessageProps {
  message: OpenZMessage;
}

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
        <div className="flex h-8 w-8 shrink-0 select-none items-center justify-center rounded-full bg-gradient-to-tr from-amber-600 to-orange-500 text-white shadow-md shadow-orange-500/20">
          <Sparkles className="h-4 w-4" />
        </div>
      )}

      <div className={`flex max-w-3xl flex-col gap-1.5 ${isUser ? 'items-end' : 'items-start'}`}>
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
          className={`relative rounded-2xl px-4 py-3 leading-relaxed shadow-sm transition-all ${
            isUser
              ? 'bg-primary text-primary-foreground rounded-tr-xs'
              : 'bg-card/70 border border-border/50 text-foreground rounded-tl-xs backdrop-blur-xs'
          }`}
        >
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
            <div className="my-1">
              {message.toolCalls.map((tool) => (
                <ToolExecutionCard key={tool.id} tool={tool} />
              ))}
            </div>
          )}

          {/* Security interception prompts (plural) */}
          {message.securityPrompts && message.securityPrompts.length > 0 && (
            <div className="my-1 space-y-1">
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
                      <div className="relative my-3 rounded-lg bg-black/80 font-mono text-xs overflow-hidden border border-border/40">
                        <div className="flex items-center justify-between border-b border-border/20 bg-muted/40 px-3 py-1 text-[10px] text-muted-foreground select-none">
                          <span>{match ? match[1] : 'code'}</span>
                          <button
                            onClick={() => {
                              navigator.clipboard.writeText(String(children).replace(/\n$/, ''));
                            }}
                            className="hover:text-foreground transition"
                          >
                            Copy code
                          </button>
                        </div>
                        <pre className="p-3 overflow-x-auto text-[12px] text-slate-200">
                          <code className={className} {...props}>
                            {children}
                          </code>
                        </pre>
                      </div>
                    ) : (
                      <code className="rounded bg-muted px-1.5 py-0.5 font-mono text-xs font-semibold text-amber-500 dark:text-amber-400" {...props}>
                        {children}
                      </code>
                    );
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

          {/* Copy Button on Hover */}
          {!message.isStreaming && (
            <button
              onClick={copyContent}
              className="absolute bottom-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity p-1 text-muted-foreground hover:text-foreground rounded bg-background/80"
              title="Copy response"
            >
              {copied ? <Check className="h-3.5 w-3.5 text-emerald-500" /> : <Copy className="h-3.5 w-3.5" />}
            </button>
          )}
        </div>
      </div>

      {isUser && (
        <div className="flex h-8 w-8 shrink-0 select-none items-center justify-center rounded-full bg-secondary text-secondary-foreground border border-border">
          <User className="h-4 w-4" />
        </div>
      )}
    </div>
  );
};
