import * as React from "react"
import {
  ArrowUp,
  CornerDownRight,
  FileText,
  Image as ImageIcon,
  Paperclip,
  Send,
  Square,
  X,
} from "lucide-react"

import { BeuiAlert } from "./alert"
import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"
import { formatFileSize } from "@/shared/lib/format"
import type { ChatAttachment } from "@/types"

export type BeuiPromptInputVariant =
  | "elevated"
  | "tweenly-ai-input"
  | "split"
  | "minimal"
  | "rare-prompt-box"
  | "kokonut-ai-prompt"
export type BeuiSendButtonVariant = "icon-round" | "arrow-round" | "icon-square"

export interface BeuiPromptInputProps {
  value: string
  onChange: (e: React.ChangeEvent<HTMLTextAreaElement>) => void
  onKeyDown?: (e: React.KeyboardEvent<HTMLTextAreaElement>) => void
  onSubmit: () => void
  onStop?: () => void
  placeholder?: string
  isStreaming?: boolean
  canSend?: boolean
  variant?: BeuiPromptInputVariant
  sendButtonVariant?: BeuiSendButtonVariant
  attachments?: ChatAttachment[]
  attachmentError?: string | null
  onDismissAttachmentError?: () => void
  onRemoveAttachment?: (id: string) => void
  onAttachClick?: () => void
  attachDisabled?: boolean
  quotedReply?: string | null
  onClearQuotedReply?: () => void
  isDragging?: boolean
  onDragOver?: (e: React.DragEvent<HTMLDivElement>) => void
  onDragLeave?: (e: React.DragEvent<HTMLDivElement>) => void
  onDrop?: (e: React.DragEvent<HTMLDivElement>) => void
  leftToolbar?: React.ReactNode
  rightToolbar?: React.ReactNode
  textareaRef?: React.RefObject<HTMLTextAreaElement | null>
  className?: string
}

/**
 * @beui/prompt-input — Multi-part composer built with shadcn/ui primitives,
 * drag-and-drop attachments, quoted reply context, and icon-only submit button.
 */
export const BeuiPromptInput: React.FC<BeuiPromptInputProps> = ({
  value,
  onChange,
  onKeyDown,
  onSubmit,
  onStop,
  placeholder = "Ask OpenZ agent anything...",
  isStreaming = false,
  canSend = false,
  variant = "elevated",
  sendButtonVariant = "icon-round",
  attachments = [],
  attachmentError = null,
  onDismissAttachmentError,
  onRemoveAttachment,
  onAttachClick,
  attachDisabled = false,
  quotedReply = null,
  onClearQuotedReply,
  isDragging = false,
  onDragOver,
  onDragLeave,
  onDrop,
  leftToolbar,
  rightToolbar,
  textareaRef,
  className,
}) => {
  const SendIcon = sendButtonVariant === "arrow-round" ? ArrowUp : Send

  const shellCls =
    variant === "split"
      ? "rounded-2xl border border-border/65 bg-background/78 shadow-lg shadow-foreground/5 backdrop-blur-xl focus-within:border-primary/55"
      : variant === "minimal"
        ? "rounded-xl border border-border/65 bg-background/75 shadow-md shadow-foreground/5 backdrop-blur-xl focus-within:border-primary/50"
        : variant === "rare-prompt-box"
          ? "rounded-3xl border border-border/60 bg-background/75 shadow-xl shadow-foreground/5 backdrop-blur-2xl focus-within:border-primary/45 focus-within:ring-2 focus-within:ring-primary/12"
          : variant === "kokonut-ai-prompt"
            ? "rounded-2xl border border-border/60 bg-background/75 shadow-lg shadow-foreground/5 backdrop-blur-xl focus-within:border-primary/45"
            : variant === "tweenly-ai-input"
              ? "rounded-[22px] border border-border/65 bg-background/75 shadow-xl shadow-foreground/5 backdrop-blur-2xl ring-1 ring-inset ring-foreground/[0.04] transition-all duration-200 focus-within:border-primary/50 focus-within:shadow-primary/10 focus-within:ring-primary/20"
              : "rounded-2xl border border-border/60 bg-background/75 shadow-lg shadow-foreground/5 backdrop-blur-xl focus-within:border-primary/55"

  return (
    <div
      onDragOver={onDragOver}
      onDragLeave={onDragLeave}
      onDrop={onDrop}
      className={cn(
        "relative flex w-full flex-col transition-colors",
        shellCls,
        isDragging && "border-primary/70 ring-2 ring-primary/20",
        className
      )}
    >
      {isDragging && (
        <div className="pointer-events-none absolute inset-0 z-20 flex items-center justify-center rounded-2xl border border-primary/60 bg-background/85">
          <div className="flex items-center gap-2 rounded-lg border border-primary/40 bg-card px-3 py-2 text-xs font-semibold text-primary shadow-md">
            <Paperclip className="size-4" />
            <span>Drop files to attach</span>
          </div>
        </div>
      )}

      {quotedReply && (
        <div className="mx-3 mt-2.5 flex items-center justify-between gap-2 rounded-xl border border-primary/25 bg-primary/10 px-3 py-1.5 text-xs text-muted-foreground">
          <div className="flex min-w-0 items-center gap-2">
            <CornerDownRight className="size-3.5 shrink-0 text-primary" />
            <span className="truncate italic text-foreground/85">{quotedReply}</span>
          </div>
          {onClearQuotedReply && (
            <Button
              type="button"
              variant="ghost"
              size="icon-xs"
              onClick={onClearQuotedReply}
              aria-label="Clear quoted reply"
            >
              <X className="size-3" />
            </Button>
          )}
        </div>
      )}

      <textarea
        ref={textareaRef}
        value={value}
        onChange={onChange}
        onKeyDown={onKeyDown}
        placeholder={placeholder}
        disabled={isStreaming}
        rows={1}
        className={cn(
          "max-h-48 w-full resize-none bg-transparent px-4 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none disabled:opacity-50",
          variant === "minimal" ? "min-h-[38px] pt-2.5 pb-1.5" : "min-h-[44px] pt-3.5 pb-2"
        )}
      />

      {(attachments.length > 0 || attachmentError) && (
        <div className="space-y-2 px-3.5 pb-2">
          {attachmentError && (
            <BeuiAlert
              tone="destructive"
              compact
              onDismiss={onDismissAttachmentError}
            >
              {attachmentError}
            </BeuiAlert>
          )}

          {attachments.length > 0 && (
            <div className="flex flex-wrap gap-2">
              {attachments.map((att) => (
                <div
                  key={att.id}
                  className="flex max-w-full items-center gap-2 rounded-lg border border-border/70 bg-muted/50 p-1.5 pr-2 text-[11px]"
                >
                  {att.mime.startsWith("image/") && att.previewUrl ? (
                    <img
                      src={att.previewUrl}
                      alt=""
                      className="size-8 shrink-0 rounded-md object-cover"
                    />
                  ) : (
                    <div className="flex size-8 shrink-0 items-center justify-center rounded-md bg-background/70 text-primary">
                      {att.mime.startsWith("image/") ? (
                        <ImageIcon className="size-4" />
                      ) : (
                        <FileText className="size-4" />
                      )}
                    </div>
                  )}
                  <div className="min-w-0">
                    <div className="max-w-40 truncate font-medium text-foreground/90">
                      {att.name}
                    </div>
                    <div className="text-[9.5px] text-muted-foreground">
                      {formatFileSize(att.size)}
                    </div>
                  </div>
                  {onRemoveAttachment && (
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-xs"
                      onClick={() => onRemoveAttachment(att.id)}
                      aria-label={`Remove ${att.name}`}
                    >
                      <X className="size-3" />
                    </Button>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      <div
        className={cn(
          "flex items-center justify-between px-3 py-2 text-xs",
          variant === "split"
            ? "rounded-b-2xl border-t border-border/60 bg-muted/25"
            : "border-t border-border/35"
        )}
      >
        <div className="flex min-w-0 flex-wrap items-center gap-1.5">
          {onAttachClick && (
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              onClick={onAttachClick}
              disabled={attachDisabled}
              className="size-7 text-muted-foreground hover:text-foreground"
              title="Attach files"
              aria-label="Attach files"
            >
              <Paperclip className="size-3.5" />
            </Button>
          )}
          {leftToolbar}
        </div>

        <div className="flex shrink-0 items-center gap-2">
          {rightToolbar}

          {isStreaming ? (
            <Button
              type="button"
              variant="destructive"
              size="icon"
              onClick={onStop}
              className={cn(
                "size-8",
                sendButtonVariant === "icon-square" ? "rounded-lg" : "rounded-full"
              )}
              title="Stop active generation"
              aria-label="Stop generation"
            >
              <Square className="size-3.5 fill-current" />
            </Button>
          ) : (
            <Button
              type="button"
              size="icon"
              onClick={onSubmit}
              disabled={!canSend}
              className={cn(
                "size-8",
                sendButtonVariant === "icon-square" ? "rounded-lg" : "rounded-full"
              )}
              title="Send message"
              aria-label="Send message"
            >
              <SendIcon className="size-3.5" />
            </Button>
          )}
        </div>
      </div>
    </div>
  )
}
