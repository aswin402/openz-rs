import React, { useState, useRef, useEffect, useMemo } from 'react';
import { useOpenZStore } from '../../store/useOpenZStore';
import { useUiCustomizationStore } from '../../store/useUiCustomizationStore';
import { ChatTrackRow } from '../../components/ChatTrackRow';
import {
  BeuiContextUsage,
  BeuiModelSelector,
  BeuiPromptInput,
} from '../../components/beui';
import { KokonutTokenAlternatives } from '../../components/registries/modernRegistries';
import { Button } from '../../components/ui/button';
import type { ChatAttachment, OpenZMessage } from '../../types';
import {
  attachmentFitsQuota,
  attachmentMimeAllowed,
  DEFAULT_ATTACHMENT_POLICY,
} from '../../types/attachments';
import { formatFileSize } from '../../shared/lib/format';
import { formatCavemanSavingsBadge } from '../../shared/lib/cavemanLevels';
import {
  estimateConversationTokens,
  mimePolicyToAcceptAttr,
  resolveModelContextWindow,
} from '../../shared/lib/runtimeMetrics';
import { cn } from '../../lib/utils';

const EMPTY_MSGS: OpenZMessage[] = [];

export interface ChatInputProps {
  embeddedInFloatingDock?: boolean;
}

export const ChatInput: React.FC<ChatInputProps> = ({
  embeddedInFloatingDock = false,
}) => {
  const [input, setInput] = useState('');
  const [attachments, setAttachments] = useState<ChatAttachment[]>([]);
  const [attachmentError, setAttachmentError] = useState<string | null>(null);
  const [isDraggingFile, setIsDraggingFile] = useState(false);

  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const capabilities = useOpenZStore((s) => s.capabilities);
  const attachmentPolicy =
    capabilities.attachments.maxCount > 0 ? capabilities.attachments : DEFAULT_ATTACHMENT_POLICY;

  const sendMessage = useOpenZStore((s) => s.sendMessage);
  const stopTurn = useOpenZStore((s) => s.stopTurn);
  const activeMessages = useOpenZStore((s) => s.messages[s.activeChatId] ?? EMPTY_MSGS);
  const isStreaming = useOpenZStore((s) => {
    const chatId = s.activeChatId;
    if (!chatId) return false;
    if (s.activeTurnIds[chatId]) return true;
    const chatMessages = s.messages[chatId] || [];
    const lastMsg = chatMessages[chatMessages.length - 1];
    return Boolean(lastMsg && lastMsg.role === 'assistant' && lastMsg.isStreaming);
  });

  const activeModel = useOpenZStore((s) => s.activeModel);
  const activeProvider = useOpenZStore((s) => s.activeProvider);
  const settings = useOpenZStore((s) => s.settings);
  const providers = useOpenZStore((s) => s.providers);
  const recentModels = useOpenZStore((s) => s.recentModels);
  const favoriteModels = useOpenZStore((s) => s.favoriteModels);
  const loadingModelProvider = useOpenZStore((s) => s.loadingModelProvider);
  const setActiveModel = useOpenZStore((s) => s.setActiveModel);
  const requestProviderModels = useOpenZStore((s) => s.requestProviderModels);
  const toggleFavoriteModel = useOpenZStore((s) => s.toggleFavoriteModel);
  const cavemanMode = useOpenZStore((s) => s.cavemanMode);
  const cavemanLevel = useOpenZStore((s) => s.cavemanLevel);
  const cavemanStats = useOpenZStore((s) => s.cavemanStats);
  const toggleCavemanMode = useOpenZStore((s) => s.toggleCavemanMode);
  const setCavemanLevel = useOpenZStore((s) => s.setCavemanLevel);
  const streamingMode = useOpenZStore((s) => s.streamingMode);
  const toggleStreamingMode = useOpenZStore((s) => s.toggleStreamingMode);

  const chatWidth = useUiCustomizationStore((s) => s.chatWidth);
  const promptInputStyle = useUiCustomizationStore((s) => s.promptInputStyle);
  const tokenAlternativesStyle = useUiCustomizationStore((s) => s.tokenAlternativesStyle);
  const sendButtonStyle = useUiCustomizationStore((s) => s.sendButtonStyle);
  const modelSelectorStyle = useUiCustomizationStore((s) => s.modelSelectorStyle);
  const contextUsageStyle = useUiCustomizationStore((s) => s.contextUsageStyle);
  const quotedReplyText = useUiCustomizationStore((s) => s.quotedReplyText);
  const setQuotedReplyText = useUiCustomizationStore((s) => s.setQuotedReplyText);

  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
      textareaRef.current.style.height = `${Math.min(textareaRef.current.scrollHeight, 180)}px`;
    }
  }, [input]);

  const estimatedTokens = useMemo(
    () => estimateConversationTokens(activeMessages, input),
    [activeMessages, input],
  );

  const maxContextTokens = useMemo(() => {
    const backendWindow =
      !settings?.model || settings.model === activeModel
        ? settings?.resolved_context_window
        : null;
    return resolveModelContextWindow(
      activeModel,
      settings?.max_tokens,
      settings?.context_limit ?? backendWindow,
    );
  }, [
    activeModel,
    settings?.model,
    settings?.max_tokens,
    settings?.context_limit,
    settings?.resolved_context_window,
  ]);

  const acceptMimeAttr = useMemo(
    () => mimePolicyToAcceptAttr(attachmentPolicy.allowedMimeTypes),
    [attachmentPolicy.allowedMimeTypes],
  );

  const handleInputChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    setInput(e.target.value);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
    if (e.key === 'Escape') {
      setAttachmentError(null);
      if (quotedReplyText) setQuotedReplyText(null);
    }
  };

  const handleSend = () => {
    if ((!input.trim() && attachments.length === 0) || isStreaming) return;
    let finalPrompt = input;
    if (quotedReplyText) {
      const quotedBlock = quotedReplyText
        .split('\n')
        .slice(0, 4)
        .map((line) => `> ${line}`)
        .join('\n');
      finalPrompt = `${quotedBlock}\n\n${input}`;
      setQuotedReplyText(null);
    }
    sendMessage(finalPrompt, attachments);
    setInput('');
    setAttachments([]);
    setAttachmentError(null);
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
    }
  };

  const addFiles = (files: FileList | File[] | null) => {
    if (!files) return;
    setAttachmentError(null);
    const incoming = Array.from(files);
    const slotsLeft = attachmentPolicy.maxCount - attachments.length;

    if (slotsLeft <= 0) {
      setAttachmentError('Attachment limit reached. Remove a file before adding another.');
      return;
    }

    const accepted = incoming.slice(0, slotsLeft);
    const rejected: string[] = [];
    let totalBytes = attachments.reduce((total, attachment) => total + attachment.size, 0);
    if (incoming.length > slotsLeft) {
      const skipped = incoming.length - slotsLeft;
      rejected.push(
        skipped +
          ' file' +
          (skipped === 1 ? '' : 's') +
          ' skipped because the limit is ' +
          attachmentPolicy.maxCount,
      );
    }

    accepted.forEach((file) => {
      if (file.size > attachmentPolicy.maxFileBytes) {
        rejected.push(file.name + ' is larger than ' + formatFileSize(attachmentPolicy.maxFileBytes));
        return;
      }
      if (!attachmentFitsQuota(totalBytes, file.size, attachmentPolicy.maxTotalBytes)) {
        rejected.push(
          file.name +
            ' would exceed the total attachment limit of ' +
            formatFileSize(attachmentPolicy.maxTotalBytes),
        );
        return;
      }
      const mime = file.type.trim().toLowerCase();
      if (!attachmentMimeAllowed(mime, attachmentPolicy.allowedMimeTypes)) {
        rejected.push(file.name + ' has an unsupported file type.');
        return;
      }
      totalBytes += file.size;

      const reader = new FileReader();
      reader.onload = () => {
        const result = String(reader.result || '');
        const data = result.includes(',') ? result.slice(result.indexOf(',') + 1) : result;
        const nextMime = file.type.trim().toLowerCase();
        setAttachments((current) => [
          ...current,
          {
            id: file.name + '-' + file.lastModified + '-' + Math.random().toString(36).slice(2),
            name: file.name,
            mime: nextMime,
            size: file.size,
            data,
            previewUrl: nextMime.startsWith('image/') ? result : undefined,
          },
        ]);
      };
      reader.onerror = () => {
        setAttachmentError('Could not read ' + file.name + '.');
      };
      reader.readAsDataURL(file);
    });

    if (rejected.length > 0) {
      setAttachmentError(rejected.join('. '));
    }
  };

  const removeAttachment = (id: string) => {
    setAttachments((current) => current.filter((item) => item.id !== id));
    setAttachmentError(null);
  };

  const handleDragOver = (event: React.DragEvent<HTMLDivElement>) => {
    if (isStreaming) return;
    event.preventDefault();
    setIsDraggingFile(true);
  };

  const handleDragLeave = (event: React.DragEvent<HTMLDivElement>) => {
    if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
      setIsDraggingFile(false);
    }
  };

  const handleDrop = (event: React.DragEvent<HTMLDivElement>) => {
    if (isStreaming) return;
    event.preventDefault();
    setIsDraggingFile(false);
    addFiles(event.dataTransfer.files);
  };

  const modelGroups = useMemo(
    () =>
      providers
        .map((p) => ({
          name: p.name,
          display: p.display,
          models: p.models,
          available: p.available !== false,
          full: p.full === true,
        }))
        .filter((g) => g.models.length > 0),
    [providers],
  );

  const canSend = !isStreaming && (input.trim().length > 0 || attachments.length > 0);
  const savingsLabel = formatCavemanSavingsBadge(cavemanStats);

  const composerBody = (
    <>
      <input
        ref={fileInputRef}
        type="file"
        multiple
        accept={acceptMimeAttr}
        className="hidden"
        onChange={(event) => {
          addFiles(event.target.files);
          event.target.value = '';
        }}
      />

      <BeuiPromptInput
        textareaRef={textareaRef}
        value={input}
        onChange={handleInputChange}
        onKeyDown={handleKeyDown}
        onSubmit={handleSend}
        onStop={stopTurn}
        isStreaming={isStreaming}
        canSend={canSend}
        variant={promptInputStyle}
        sendButtonVariant={sendButtonStyle}
        attachments={attachments}
        attachmentError={attachmentError}
        onDismissAttachmentError={() => setAttachmentError(null)}
        onRemoveAttachment={removeAttachment}
        onAttachClick={() => fileInputRef.current?.click()}
        attachDisabled={isStreaming || attachments.length >= attachmentPolicy.maxCount}
        quotedReply={quotedReplyText}
        onClearQuotedReply={() => setQuotedReplyText(null)}
        isDragging={isDraggingFile}
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
        leftToolbar={
          <>
            <BeuiModelSelector
              activeModel={activeModel}
              activeProvider={activeProvider}
              groups={modelGroups}
              favorites={favoriteModels}
              recents={recentModels}
              loadingProvider={loadingModelProvider}
              variant={modelSelectorStyle}
              onSelectModel={(provider, model) => setActiveModel(model, provider)}
              onToggleFavorite={toggleFavoriteModel}
              onLoadAllForProvider={requestProviderModels}
            />

            <KokonutTokenAlternatives
              enabled={cavemanMode}
              activeLevel={cavemanLevel}
              savingsBadge={savingsLabel ? `⛏${savingsLabel}` : null}
              variant={tokenAlternativesStyle}
              onToggleEnabled={toggleCavemanMode}
              onSelectLevel={(level) => setCavemanLevel(level)}
            />

            <Button
              type="button"
              variant={streamingMode ? 'outline' : 'ghost'}
              size="xs"
              onClick={toggleStreamingMode}
              className={cn(
                'h-6 rounded-full px-2.5 text-[10px]',
                streamingMode
                  ? 'border-primary/35 bg-primary/10 text-primary hover:bg-primary/20 hover:text-primary'
                  : 'text-muted-foreground',
              )}
              title="Toggle live token streaming"
            >
              <span>Streaming {streamingMode ? 'ON' : 'OFF'}</span>
            </Button>
          </>
        }
        rightToolbar={
          <BeuiContextUsage
            usedTokens={estimatedTokens}
            maxTokens={maxContextTokens}
            messageCount={activeMessages.length}
            variant={contextUsageStyle}
          />
        }
      />
    </>
  );

  if (embeddedInFloatingDock) {
    return composerBody;
  }

  return (
    <ChatTrackRow chatWidth={chatWidth} className="pb-3.5" centerClassName="relative">
      {composerBody}
    </ChatTrackRow>
  );
};
