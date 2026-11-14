import React, { useMemo, useState } from 'react';
import ReactMarkdown from 'react-markdown';
import type { Components } from 'react-markdown';
import remarkGfm from 'remark-gfm';
import rehypeHighlight from 'rehype-highlight';
import rehypeKatex from 'rehype-katex';
import type { ActivityNotice, OpenZMessage } from '../../types';
import { ToolExecutionCard } from './ToolExecutionCard';
import { buildMessageActivitySteps } from '../activity/activityAdapters';
import { SecurityGuardPrompt } from '../../components/SecurityGuardPrompt';
import { pairSecurityPromptsWithToolCalls } from '../../components/assistant-ui/elements';
import { ChatTrackRow } from '../../components/ChatTrackRow';
import {
  AgentActivity,
  BeuiAgentAvatar,
  BeuiAlert,
  BeuiCitations,
  BeuiCodeBlock,
  BeuiDataTable,
  BeuiFileDiff,
  BeuiMessageActions,
  BeuiTelemetryNoticeStrip,
  BeuiUserAvatar,
  ImageGeneration,
  ReasoningText,
  StreamingResponse,
  extractAllMarkdownSources,
  type AgentActivityStep,
  type BeuiActivityPlacement,
} from '../../components/beui';
import { Badge } from '../../components/ui/badge';
import { Button } from '../../components/ui/button';
import { Textarea } from '../../components/ui/textarea';
import { useOpenZStore } from '../../store/useOpenZStore';
import { resolveNoticeJumpAction } from '../../store/useToastStore';
import { useUiCustomizationStore } from '../../store/useUiCustomizationStore';
import { extractCodeLanguage, formatFileSize } from '../../shared/lib/format';
import { cn } from '../../lib/utils';
import {
  FileText,
  Image as ImageIcon,
  Send,
  X,
} from 'lucide-react';
import 'katex/dist/katex.min.css';

interface ChatMessageProps {
  message: OpenZMessage;
}

interface HastLikeNode {
  type?: string;
  tagName?: string;
  value?: string;
  children?: HastLikeNode[];
}

function extractHastText(node: HastLikeNode | undefined): string {
  if (!node) return '';
  if (node.type === 'text' && typeof node.value === 'string') {
    return node.value;
  }
  if (!Array.isArray(node.children)) return '';
  return node.children.map((child) => extractHastText(child)).join('');
}

function extractHastTable(rawNode: unknown): { headers: string[]; rows: string[][] } | null {
  const node = rawNode as HastLikeNode | undefined;
  if (!node || !Array.isArray(node.children)) return null;

  const headers: string[] = [];
  const rows: string[][] = [];

  const collectCells = (tr: HastLikeNode): string[] => {
    if (!Array.isArray(tr.children)) return [];
    return tr.children
      .filter((cell) => cell.tagName === 'th' || cell.tagName === 'td')
      .map((cell) => extractHastText(cell).trim());
  };

  for (const section of node.children) {
    if (section.tagName === 'thead' && Array.isArray(section.children)) {
      for (const tr of section.children) {
        if (tr.tagName === 'tr' && headers.length === 0) {
          headers.push(...collectCells(tr));
        }
      }
    } else if (section.tagName === 'tbody' && Array.isArray(section.children)) {
      for (const tr of section.children) {
        if (tr.tagName === 'tr') {
          const cells = collectCells(tr);
          if (cells.length > 0) rows.push(cells);
        }
      }
    } else if (section.tagName === 'tr') {
      const cells = collectCells(section);
      if (cells.length > 0) {
        if (headers.length === 0) headers.push(...cells);
        else rows.push(cells);
      }
    }
  }

  if (headers.length < 2 || rows.length === 0) return null;
  return { headers, rows };
}

function looksLikeUnifiedDiffBlock(language: string | undefined, text: string): boolean {
  const lang = (language ?? '').toLowerCase();
  if (lang === 'diff' || lang === 'patch') return true;
  if (text.startsWith('diff --git ')) return true;
  if (
    (text.startsWith('--- ') || text.includes('\n--- ')) &&
    text.includes('\n+++ ') &&
    text.includes('\n@@ ')
  ) {
    return true;
  }
  return false;
}

const INLINE_PLACEMENT_PREDICATES: Record<
  BeuiActivityPlacement,
  (
    msg: OpenZMessage,
    steps: ReadonlyArray<AgentActivityStep>,
    working: boolean,
  ) => boolean
> = {
  'panel-only': () => false,
  'streaming-only': (_msg, _steps, working) => working,
  'always-inline': (_msg, steps, working) => steps.length > 0 || working,
  'beui-timeline': (_msg, steps, working) => steps.length > 0 || working,
};

export const ChatMessage: React.FC<ChatMessageProps> = React.memo(({ message }) => {
  const handleSecurityChoice = useOpenZStore((s) => s.handleSecurityChoice);
  const regenerateMessage = useOpenZStore((s) => s.regenerateMessage);
  const editAndResendMessage = useOpenZStore((s) => s.editAndResendMessage);
  const newSession = useOpenZStore((s) => s.newSession);
  const setActiveView = useOpenZStore((s) => s.setActiveView);
  const setIsMemoryOpen = useOpenZStore((s) => s.setIsMemoryOpen);
  const setIsActivityPanelOpen = useOpenZStore((s) => s.setIsActivityPanelOpen);
  const setIsLogsOpen = useOpenZStore((s) => s.setIsLogsOpen);

  const chatWidth = useUiCustomizationStore((s) => s.chatWidth);
  const thinkingAnimation = useUiCustomizationStore((s) => s.thinkingAnimation);
  const streamingAnimation = useUiCustomizationStore((s) => s.streamingAnimation);
  const agentAvatar = useUiCustomizationStore((s) => s.agentAvatar);
  const userAvatar = useUiCustomizationStore((s) => s.userAvatar);
  const bubbleStyle = useUiCustomizationStore((s) => s.bubbleStyle);
  const codeStyle = useUiCustomizationStore((s) => s.codeStyle);
  const diffLayout = useUiCustomizationStore((s) => s.diffLayout);
  const citationsStyle = useUiCustomizationStore((s) => s.citationsStyle);
  const dataTableStyle = useUiCustomizationStore((s) => s.dataTableStyle);
  const imageGenStyle = useUiCustomizationStore((s) => s.imageGenStyle);
  const activityStyle = useUiCustomizationStore((s) => s.activityStyle);
  const activityVariant = useUiCustomizationStore((s) => s.activityVariant);
  const activityOrbStyle = useUiCustomizationStore((s) => s.activityOrbStyle);
  const activityDensity = useUiCustomizationStore((s) => s.activityDensity);
  const activityMaxHeight = useUiCustomizationStore((s) => s.activityMaxHeight);
  const activityAutoCollapse = useUiCustomizationStore((s) => s.activityAutoCollapse);
  const activityShowDetails = useUiCustomizationStore((s) => s.activityShowDetails);
  const inlineNoticeStyle = useUiCustomizationStore((s) => s.inlineNoticeStyle);
  const approvalStyle = useUiCustomizationStore((s) => s.approvalStyle);
  const alertStyle = useUiCustomizationStore((s) => s.alertStyle);
  const messageActionsStyle = useUiCustomizationStore((s) => s.messageActionsStyle);
  const showMessageActions = useUiCustomizationStore((s) => s.showMessageActions);
  const setQuotedReplyText = useUiCustomizationStore((s) => s.setQuotedReplyText);

  const [isEditing, setIsEditing] = useState(false);
  const [draftEdit, setDraftEdit] = useState(message.content);

  const isUser = message.role === 'user';

  const markdownComponents = useMemo<Components>(
    () => ({
      code({ node, className, children, ...props }) {
        void node;
        const language = extractCodeLanguage(className);
        const text = String(children);
        const isBlock = Boolean(language) || text.includes('\n');

        if (!isBlock) {
          return (
            <code
              className="rounded bg-muted px-1.5 py-0.5 font-mono text-xs font-semibold text-primary"
              {...props}
            >
              {children}
            </code>
          );
        }

        if (looksLikeUnifiedDiffBlock(language, text)) {
          return (
            <div className="my-3 w-full not-prose">
              <BeuiFileDiff patch={text} defaultView={diffLayout} />
            </div>
          );
        }

        return (
          <div className="my-3 w-full not-prose">
            <BeuiCodeBlock
              language={language || 'text'}
              code={text}
              variant={codeStyle}
              showLineNumbers={codeStyle !== 'compact'}
            />
          </div>
        );
      },
      img({ src, alt }) {
        if (!src) return null;
        return (
          <span className="block not-prose">
            <ImageGeneration
              src={src}
              prompt={alt || undefined}
              status="completed"
              variant={imageGenStyle}
            />
          </span>
        );
      },
      table({ node, children }) {
        const parsed = extractHastTable(node);
        if (parsed) {
          return (
            <div className="my-3 w-full not-prose">
              <BeuiDataTable
                headers={parsed.headers}
                rows={parsed.rows}
                variant={dataTableStyle}
              />
            </div>
          );
        }
        return (
          <div className="my-4 w-full overflow-x-auto rounded-xl border border-border/80 bg-card/40">
            <table className="min-w-[420px] w-full border-collapse text-left text-xs text-foreground/90">
              {children}
            </table>
          </div>
        );
      },
    }),
    [codeStyle, diffLayout, imageGenStyle, dataTableStyle],
  );

  const citationData = useMemo(() => {
    if (isUser || !message.content) {
      return { bodyText: message.content, refs: [] };
    }
    return extractAllMarkdownSources(message.content);
  }, [isUser, message.content]);

  const hasActiveTool = Boolean(
    message.toolCalls?.some(
      (tc) => tc.status === 'running' || tc.status === 'awaiting_approval',
    ),
  );
  const hasPendingGuard = Boolean(
    message.securityPrompts?.some((sp) => sp.status === 'pending'),
  );
  const isActivityWorking =
    !isUser &&
    (hasActiveTool ||
      hasPendingGuard ||
      (Boolean(message.isStreaming) && message.content.trim().length === 0));

  const inlineActivitySteps = useMemo(() => {
    if (isUser || activityStyle === 'panel-only') return [];
    return buildMessageActivitySteps([message], {
      chronological: true,
      limit: 64,
    });
  }, [isUser, activityStyle, message]);

  const showInlineActivity =
    !isUser &&
    INLINE_PLACEMENT_PREDICATES[activityStyle](
      message,
      inlineActivitySteps,
      isActivityWorking,
    );

  const coordinatedToolsAndApprovals = useMemo(() => {
    if (isUser) {
      return { visibleToolCalls: [], visibleApprovals: [] };
    }
    return pairSecurityPromptsWithToolCalls(
      message.toolCalls,
      message.securityPrompts,
      { keepApprovedReceipt: approvalStyle === 'assistant-receipt' },
    );
  }, [isUser, message.toolCalls, message.securityPrompts, approvalStyle]);

  const { visibleToolCalls, visibleApprovals } = coordinatedToolsAndApprovals;

  const visibleTelemetryNotices = useMemo<ActivityNotice[]>(() => {
    if (isUser || !message.activityNotices || message.activityNotices.length === 0) {
      return [];
    }
    return message.activityNotices.filter(
      (notice) => !(notice.kind === 'system' && notice.title === 'Progress'),
    );
  }, [isUser, message.activityNotices]);

  const handleInspectNotice = React.useCallback(
    (notice: ActivityNotice) => {
      const jump = resolveNoticeJumpAction(notice.kind, notice.title, {
        openSkills: () => setActiveView('skills'),
        openKnowledge: () => setActiveView('knowledge'),
        openMemory: () => setIsMemoryOpen(true),
        openActivity: () => {
          setActiveView('chats');
          setIsActivityPanelOpen(true);
        },
        openAgents: () => setActiveView('agents'),
        openLogs: () => setIsLogsOpen(true),
        focusChat: () => setActiveView('chats'),
      });
      jump.onAction();
    },
    [setActiveView, setIsActivityPanelOpen, setIsLogsOpen, setIsMemoryOpen],
  );

  if (message.isNotice || message.role === 'system' || message.role === 'tool') {
    if (!message.content && visibleTelemetryNotices.length > 0) {
      if (inlineNoticeStyle === 'toast-only') return null;
      return (
        <ChatTrackRow chatWidth={chatWidth} className="py-0.5">
          <BeuiTelemetryNoticeStrip
            notices={visibleTelemetryNotices}
            styleVariant={inlineNoticeStyle}
            onInspectNotice={handleInspectNotice}
          />
        </ChatTrackRow>
      );
    }
    if (!message.content) return null;
    return (
      <ChatTrackRow chatWidth={chatWidth} className="py-1">
        <BeuiAlert tone="info" compact={alertStyle === 'compact-pill'}>
          {message.content}
        </BeuiAlert>
      </ChatTrackRow>
    );
  }

  const hasWideAssistantBlocks = Boolean(
    !isUser &&
      (showInlineActivity ||
        visibleToolCalls.length > 0 ||
        visibleApprovals.length > 0 ||
        message.content.includes('```')),
  );

  const bubbleClasses = (() => {
    switch (bubbleStyle) {
      case 'editorial':
        return isUser
          ? 'bg-muted/70 text-foreground border border-border/60 rounded-xl px-4 py-2.5'
          : 'bg-transparent border-0 text-foreground px-0.5 py-1 w-full';
      case 'beui-full':
        return isUser
          ? 'bg-primary text-primary-foreground rounded-2xl rounded-tr-xs px-4 py-2.5 shadow-xs'
          : 'w-full bg-card border border-border/75 text-foreground rounded-2xl rounded-tl-xs px-4 py-3 shadow-xs';
      case 'beui-card':
      default:
        return isUser
          ? 'bg-primary text-primary-foreground rounded-2xl rounded-tr-xs px-4 py-2.5 shadow-xs'
          : cn(
              'bg-card border border-border/75 text-foreground rounded-2xl rounded-tl-xs px-4 py-3 shadow-xs',
              hasWideAssistantBlocks && 'w-full',
            );
    }
  })();

  const renderAssistantAvatar = () => {
    if (agentAvatar === 'none') return null;
    return <BeuiAgentAvatar variant={agentAvatar} className="mt-5" />;
  };

  const renderUserAvatar = () => {
    if (userAvatar === 'none') return null;
    return <BeuiUserAvatar variant={userAvatar} className="mt-5" />;
  };

  const submitEditedMessage = () => {
    const trimmed = draftEdit.trim();
    if (!trimmed) return;
    setIsEditing(false);
    editAndResendMessage(message.id, trimmed);
  };

  const citationsFooter =
    !isUser && citationData.refs.length > 0 ? (
      <div className="not-prose">
        <BeuiCitations
          refs={citationData.refs}
          variant={citationsStyle}
          defaultOpen={citationsStyle === 'cards'}
        />
      </div>
    ) : undefined;

  return (
    <ChatTrackRow
      chatWidth={chatWidth}
      leftGutter={!isUser ? renderAssistantAvatar() : undefined}
      rightGutter={isUser ? renderUserAvatar() : undefined}
      className="t-msg-enter group py-2 text-sm"
    >
      <div
        className={cn(
          'flex w-full min-w-0 flex-col gap-1.5',
          isUser ? 'items-end' : 'items-start',
        )}
      >
        <div className="flex select-none items-center gap-2 text-[11px] font-medium text-muted-foreground">
          <span>{isUser ? 'You' : 'OpenZ Agent'}</span>
          {message.model && (
            <Badge variant="secondary" className="px-1.5 py-0 font-mono text-[10px]">
              {message.model}
            </Badge>
          )}
          <span className="text-[10px] opacity-70">
            {new Date(message.timestamp).toLocaleTimeString([], {
              hour: '2-digit',
              minute: '2-digit',
            })}
          </span>
        </div>

        {isEditing && isUser ? (
          <div className="w-full rounded-2xl border border-primary/40 bg-card p-3 shadow-xs">
            <Textarea
              value={draftEdit}
              onChange={(e) => setDraftEdit(e.target.value)}
              rows={Math.min(8, Math.max(2, draftEdit.split('\n').length))}
              aria-label="Edit message text"
            />
            <div className="mt-2 flex items-center justify-end gap-2">
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={() => {
                  setIsEditing(false);
                  setDraftEdit(message.content);
                }}
              >
                <X className="size-3" />
                <span>Cancel</span>
              </Button>
              <Button
                type="button"
                size="sm"
                onClick={submitEditedMessage}
                disabled={!draftEdit.trim()}
              >
                <Send className="size-3" />
                <span>Save &amp; Resend</span>
              </Button>
            </div>
          </div>
        ) : (
          <div
            className={cn(
              'relative max-w-full min-w-0 leading-relaxed',
              !isUser && (bubbleStyle === 'beui-full' || hasWideAssistantBlocks || bubbleStyle === 'editorial')
                ? 'w-full'
                : 'w-fit',
              bubbleClasses,
            )}
          >
            {visibleTelemetryNotices.length > 0 && (
              <BeuiTelemetryNoticeStrip
                notices={visibleTelemetryNotices}
                styleVariant={inlineNoticeStyle}
                onInspectNotice={handleInspectNotice}
                className="mb-2"
              />
            )}

            {!showInlineActivity && !message.content && message.isStreaming && (
              <div className="select-none py-0.5">
                <ReasoningText isStreaming label="Thinking…" variant={thinkingAnimation} />
              </div>
            )}

            {showInlineActivity && (
              <AgentActivity
                steps={inlineActivitySteps}
                isActive={isActivityWorking}
                contentType={inlineActivitySteps.length === 0 ? 'step' : 'mixed'}
                mode="inline-collapsible"
                variant={activityVariant}
                orbStyle={activityOrbStyle}
                density={activityDensity}
                maxHeight={activityMaxHeight}
                autoCollapse={activityAutoCollapse}
                showDetails={activityShowDetails}
                startedAt={message.timestamp}
                className={message.content ? 'mb-2' : undefined}
              />
            )}

            {visibleToolCalls.length > 0 && (
              <div className="my-1.5 flex w-full min-w-0 flex-col gap-1 not-prose">
                {visibleToolCalls.map((tool) => (
                  <ToolExecutionCard key={tool.id} tool={tool} />
                ))}
              </div>
            )}

            {visibleApprovals.length > 0 && (
              <div className="my-2 flex w-full min-w-0 flex-col gap-2.5 not-prose">
                {visibleApprovals.map((item) => (
                  <SecurityGuardPrompt
                    key={item.prompt.id}
                    prompt={item.prompt}
                    stateOverride={item.state}
                    onChoice={(choice) => handleSecurityChoice(item.prompt.id, choice)}
                  />
                ))}
              </div>
            )}

            {message.attachments && message.attachments.length > 0 && (
              <div className="mb-3 flex flex-wrap gap-2">
                {message.attachments.map((attachment) => (
                  <div
                    key={attachment.id}
                    className="flex max-w-full items-center gap-2 rounded-lg border border-border/70 bg-muted/50 p-1.5 text-[10px] text-muted-foreground"
                  >
                    {attachment.previewUrl && attachment.mime.startsWith('image/') ? (
                      <img
                        src={attachment.previewUrl}
                        alt=""
                        className="h-8 w-8 shrink-0 rounded-md object-cover"
                      />
                    ) : (
                      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-background/70 text-primary">
                        {attachment.mime.startsWith('image/') ? (
                          <ImageIcon className="h-4 w-4" />
                        ) : (
                          <FileText className="h-4 w-4" />
                        )}
                      </div>
                    )}
                    <div className="min-w-0">
                      <div className="max-w-48 truncate font-medium text-foreground/90">
                        {attachment.name}
                      </div>
                      <div className="text-[9px] text-muted-foreground">
                        {formatFileSize(attachment.size)}
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            )}

            {message.content && (
              <div className="prose dark:prose-invert prose-xs max-w-none break-words">
                {isUser ? (
                  <ReactMarkdown
                    remarkPlugins={[remarkGfm]}
                    rehypePlugins={[rehypeHighlight, rehypeKatex]}
                    components={markdownComponents}
                  >
                    {message.content}
                  </ReactMarkdown>
                ) : (
                  <StreamingResponse
                    isStreaming={Boolean(message.isStreaming)}
                    cursorStyle={streamingAnimation}
                    footer={citationsFooter}
                  >
                    <ReactMarkdown
                      remarkPlugins={[remarkGfm]}
                      rehypePlugins={[rehypeHighlight, rehypeKatex]}
                      components={markdownComponents}
                    >
                      {citationData.refs.length > 0 ? citationData.bodyText : message.content}
                    </ReactMarkdown>
                  </StreamingResponse>
                )}
              </div>
            )}
          </div>
        )}

        {!message.isStreaming && !isEditing && showMessageActions && (
          <div className="mt-0.5 select-none px-0.5 opacity-0 transition-opacity duration-150 group-hover:opacity-100 focus-within:opacity-100">
            <BeuiMessageActions
              text={message.content}
              isUser={isUser}
              variant={messageActionsStyle}
              onRegenerate={!isUser ? () => regenerateMessage(message.id) : undefined}
              onEdit={
                isUser
                  ? () => {
                      setDraftEdit(message.content);
                      setIsEditing(true);
                    }
                  : undefined
              }
              onReply={() => setQuotedReplyText(message.content)}
              onFork={!isUser ? () => newSession() : undefined}
            />
          </div>
        )}
      </div>
    </ChatTrackRow>
  );
});
