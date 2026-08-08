import React, { useState, useRef, useEffect, useMemo } from 'react';
import { useOpenZStore } from '../store/useOpenZStore';
import { Send, Square, Zap, ChevronsUpDown, Loader2, Paperclip, X, FileText, Image as ImageIcon, AlertTriangle } from 'lucide-react';
import type { ChatAttachment } from '../types';

const MAX_ATTACHMENTS = 8;
const MAX_ATTACHMENT_BYTES = 15 * 1024 * 1024;

function formatFileSize(bytes: number): string {
  if (bytes < 1024 * 1024) return Math.max(1, Math.round(bytes / 1024)) + ' KB';
  return (bytes / (1024 * 1024)).toFixed(bytes < 10 * 1024 * 1024 ? 1 : 0) + ' MB';
}

function isImageAttachment(attachment: ChatAttachment): boolean {
  return attachment.mime.startsWith('image/') && Boolean(attachment.previewUrl);
}

export const ChatInput: React.FC = () => {
  const [input, setInput] = useState('');
  const [showCommands, setShowCommands] = useState(false);
  const [showModelPicker, setShowModelPicker] = useState(false);
  const [filteredCmds, setFilteredCmds] = useState<{ cmd: string; desc: string }[]>([]);
  const [attachments, setAttachments] = useState<ChatAttachment[]>([]);
  const [attachmentError, setAttachmentError] = useState<string | null>(null);
  const [isDraggingFile, setIsDraggingFile] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const modelButtonRef = useRef<HTMLButtonElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);

  const sendMessage = useOpenZStore((s) => s.sendMessage);
  const stopTurn = useOpenZStore((s) => s.stopTurn);
  const isStreaming = useOpenZStore((s) => s.isStreaming);
  const activeModel = useOpenZStore((s) => s.activeModel);
  const providers = useOpenZStore((s) => s.providers);
  const setActiveModel = useOpenZStore((s) => s.setActiveModel);
  const slashCommands = useOpenZStore((s) => s.slashCommands);
  const cavemanMode = useOpenZStore((s) => s.cavemanMode);
  const toggleCavemanMode = useOpenZStore((s) => s.toggleCavemanMode);
  const streamingMode = useOpenZStore((s) => s.streamingMode);
  const toggleStreamingMode = useOpenZStore((s) => s.toggleStreamingMode);

  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
      textareaRef.current.style.height = `${Math.min(textareaRef.current.scrollHeight, 180)}px`;
    }
  }, [input]);

  useEffect(() => {
    const onClickOutside = (e: MouseEvent) => {
      if (modelButtonRef.current?.contains(e.target as Node)) return;
      if (popoverRef.current?.contains(e.target as Node)) return;
      setShowModelPicker(false);
    };
    document.addEventListener('mousedown', onClickOutside);
    return () => document.removeEventListener('mousedown', onClickOutside);
  }, []);

  const handleInputChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const val = e.target.value;
    setInput(val);

    if (val.startsWith('/')) {
      setShowCommands(true);
      const query = val.toLowerCase();
      const cmds = slashCommands.length > 0 ? slashCommands : [{ cmd: '/help', desc: 'Show available slash commands' }];
      setFilteredCmds(cmds.filter((item) => item.cmd.toLowerCase().includes(query)));
    } else {
      setShowCommands(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
    if (e.key === 'Escape') {
      setShowCommands(false);
      setShowModelPicker(false);
      setAttachmentError(null);
    }
  };

  const handleSend = () => {
    if ((!input.trim() && attachments.length === 0) || isStreaming) return;
    sendMessage(input, attachments);
    setInput('');
    setAttachments([]);
    setAttachmentError(null);
    setShowCommands(false);
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
    }
  };

  const addFiles = (files: FileList | File[] | null) => {
    if (!files) return;
    setAttachmentError(null);
    const incoming = Array.from(files);
    const slotsLeft = MAX_ATTACHMENTS - attachments.length;

    if (slotsLeft <= 0) {
      setAttachmentError('Attachment limit reached. Remove a file before adding another.');
      return;
    }

    const accepted = incoming.slice(0, slotsLeft);
    const rejected: string[] = [];
    if (incoming.length > slotsLeft) {
      const skipped = incoming.length - slotsLeft;
      rejected.push(skipped + ' file' + (skipped === 1 ? '' : 's') + ' skipped because the limit is ' + MAX_ATTACHMENTS);
    }

    accepted.forEach((file) => {
      if (file.size > MAX_ATTACHMENT_BYTES) {
        rejected.push(file.name + ' is larger than ' + formatFileSize(MAX_ATTACHMENT_BYTES));
        return;
      }

      const reader = new FileReader();
      reader.onload = () => {
        const result = String(reader.result || '');
        const data = result.includes(',') ? result.slice(result.indexOf(',') + 1) : result;
        const mime = file.type || 'application/octet-stream';
        setAttachments((current) => [
          ...current,
          {
            id: file.name + '-' + file.lastModified + '-' + Math.random().toString(36).slice(2),
            name: file.name,
            mime,
            size: file.size,
            data,
            previewUrl: mime.startsWith('image/') ? result : undefined,
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

  const selectCommand = (cmd: string) => {
    setInput(cmd);
    setShowCommands(false);
    if (textareaRef.current) textareaRef.current.focus();
  };

  // Flatten providers → models for the picker, grouped by provider display name.
  const modelGroups = useMemo(
    () =>
      providers
        .map((p) => ({ name: p.name, display: p.display, models: p.models }))
        .filter((g) => g.models.length > 0),
    [providers],
  );

  const currentModelLabel = useMemo(() => {
    if (!activeModel) return 'Select model';
    for (const g of modelGroups) {
      if (g.models.includes(activeModel)) return activeModel.split('/').pop() || activeModel;
    }
    return activeModel.split('/').pop() || activeModel;
  }, [activeModel, modelGroups]);

  const canSend = !isStreaming && (input.trim().length > 0 || attachments.length > 0);

  return (
    <div className="relative w-full max-w-3xl mx-auto px-4 pb-4">
      {/* Fade gradient shadow above the input - removed */}
      {/* Slash Commands Suggestion Popover */}
      {showCommands && filteredCmds.length > 0 && (
        <div className="absolute bottom-full left-4 right-4 mb-2 max-h-56 overflow-y-auto rounded-xl border border-border bg-card/95 p-1.5 shadow-xl backdrop-blur-md z-30">
          <div className="px-2 py-1 text-[10px] font-semibold text-muted-foreground uppercase tracking-wider">
            OpenZ Slash Commands
          </div>
          {filteredCmds.map((item) => (
            <button
              key={item.cmd}
              onClick={() => selectCommand(item.cmd)}
              className="flex w-full items-center justify-between rounded-lg px-2.5 py-1.5 text-left text-xs hover:bg-muted/70 transition"
            >
              <span className="font-mono font-semibold text-amber-500">{item.cmd}</span>
              <span className="text-[11px] text-muted-foreground">{item.desc}</span>
            </button>
          ))}
        </div>
      )}

      {/* Model Picker Popover */}
      {showModelPicker && (
        <div
          ref={popoverRef}
          className="absolute bottom-full left-4 mb-2 w-72 overflow-hidden rounded-xl border border-border bg-card/95 shadow-xl backdrop-blur-md z-30"
        >
          <div className="border-b border-border/40 px-3 py-2 text-[10px] font-semibold text-muted-foreground uppercase tracking-wider">
            Available Models
          </div>
          <div className="max-h-64 overflow-y-auto p-1.5">
            {modelGroups.length === 0 ? (
              <div className="px-2.5 py-2 text-[11px] text-muted-foreground">
                No models loaded — start the gateway to fetch the model list.
              </div>
            ) : (
              modelGroups.map((g) => (
                <div key={g.name}>
                  <div className="px-2.5 py-1 text-[10px] font-semibold text-muted-foreground/80">
                    {g.display || g.name}
                  </div>
                  {g.models.map((model) => (
                    <button
                      key={model}
                      onClick={() => {
                        setActiveModel(model, g.name);
                        setShowModelPicker(false);
                      }}
                      className={`flex w-full items-center justify-between rounded-lg px-2.5 py-1.5 text-left text-xs transition hover:bg-muted/70 ${
                        model === activeModel ? 'bg-amber-500/10 text-amber-400' : 'text-foreground/90'
                      }`}
                    >
                      <span className="truncate font-mono">{model}</span>
                      {model === activeModel && <span className="text-[10px] text-amber-400">active</span>}
                    </button>
                  ))}
                </div>
              ))
            )}
          </div>
        </div>
      )}

      {/* Main Input Card */}
      <div
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
        className={'relative flex flex-col rounded-2xl border border-border/80 bg-card/90 shadow-xl backdrop-blur-md transition-all focus-within:border-amber-500/50 focus-within:ring-2 focus-within:ring-amber-500/20 ' + (isDraggingFile ? 'border-amber-500/70 ring-2 ring-amber-500/25' : '')}
      >
        {isDraggingFile && (
          <div className="pointer-events-none absolute inset-0 z-20 flex items-center justify-center rounded-2xl border border-amber-500/60 bg-background/80 backdrop-blur-sm">
            <div className="flex items-center gap-2 rounded-lg border border-amber-500/40 bg-card px-3 py-2 text-xs font-semibold text-amber-500 shadow-lg">
              <Paperclip className="h-4 w-4" /> Drop files to attach
            </div>
          </div>
        )}
        <textarea
          ref={textareaRef}
          value={input}
          onChange={handleInputChange}
          onKeyDown={handleKeyDown}
          placeholder="Ask OpenZ agent anything or type '/' for slash commands..."
          rows={1}
          aria-label="Message input"
          className="w-full resize-none bg-transparent p-3.5 text-sm text-foreground placeholder:text-muted-foreground/60 focus:outline-none"
        />

        {(attachments.length > 0 || attachmentError) && (
          <div className="space-y-2 px-3 pb-2">
            {attachmentError && (
              <div className="flex items-start gap-2 rounded-lg border border-red-500/30 bg-red-500/10 px-2 py-1.5 text-[11px] text-red-300">
                <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
                <span>{attachmentError}</span>
              </div>
            )}
            {attachments.length > 0 && (
              <div className="flex flex-wrap gap-2" aria-label="Attachments">
                {attachments.map((attachment) => (
                  <div key={attachment.id} className="group flex max-w-full items-center gap-2 rounded-lg border border-border/70 bg-muted/50 p-1.5 text-[10px]">
                    {isImageAttachment(attachment) ? (
                      <img src={attachment.previewUrl} alt="" className="h-9 w-9 shrink-0 rounded-md object-cover" />
                    ) : (
                      <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-md bg-background/70 text-amber-500">
                        {attachment.mime.startsWith('image/') ? <ImageIcon className="h-4 w-4" /> : <FileText className="h-4 w-4" />}
                      </div>
                    )}
                    <div className="min-w-0">
                      <div className="max-w-40 truncate font-medium text-foreground/90">{attachment.name}</div>
                      <div className="text-[9px] text-muted-foreground">{formatFileSize(attachment.size)}</div>
                    </div>
                    <button type="button" onClick={() => removeAttachment(attachment.id)} className="rounded p-1 text-muted-foreground opacity-70 hover:bg-background hover:text-foreground group-hover:opacity-100" aria-label={'Remove ' + attachment.name}>
                      <X className="h-3 w-3" />
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {/* Toolbar & Actions Footer */}
        <div className="flex items-center justify-between px-3 py-2 border-t border-border/30 text-xs">
          <div className="flex min-w-0 items-center gap-2">
            <input ref={fileInputRef} type="file" multiple accept="image/*,.pdf,.txt,.md,.json,.csv,.rs,.ts,.tsx,.js,.jsx,.py,.toml,.yaml,.yml" className="hidden" onChange={(event) => { addFiles(event.target.files); event.target.value = ''; }} />
            <button type="button" onClick={() => fileInputRef.current?.click()} disabled={isStreaming || attachments.length >= MAX_ATTACHMENTS} className="rounded-lg p-1.5 text-muted-foreground hover:bg-muted hover:text-foreground disabled:opacity-40" title="Attach files" aria-label="Attach files">
              <Paperclip className="h-3.5 w-3.5" />
            </button>
            {/* Model selector (real data from models_list) */}
            <button
              ref={modelButtonRef}
              onClick={() => setShowModelPicker(!showModelPicker)}
              className="flex max-w-[180px] items-center gap-1 rounded-lg bg-muted/60 px-2 py-1 font-mono text-[10px] text-foreground/90 hover:bg-muted transition select-none"
              title="Switch active model"
            >
              {activeModel ? (
                <>
                  <span className="truncate">{currentModelLabel}</span>
                  <ChevronsUpDown className="h-3 w-3 shrink-0 text-amber-500" />
                </>
              ) : (
                <>
                  <Loader2 className="h-3 w-3 animate-spin text-amber-500" />
                  <span className="text-muted-foreground">model</span>
                </>
              )}
            </button>

            <button
              onClick={toggleCavemanMode}
              className={`flex items-center gap-1 rounded-lg px-2 py-1 text-[10px] font-medium transition select-none ${
                cavemanMode
                  ? 'bg-amber-500/20 text-amber-400 border border-amber-500/30'
                  : 'bg-muted/40 text-muted-foreground hover:bg-muted'
              }`}
              title="Caveman mode strips filler words for maximum speed"
            >
              <Zap className="h-3 w-3" /> Caveman {cavemanMode ? 'ON' : 'OFF'}
            </button>

            <button
              onClick={toggleStreamingMode}
              className={`flex items-center gap-1 rounded-lg px-2 py-1 text-[10px] font-medium transition select-none ${
                streamingMode
                  ? 'bg-amber-500/20 text-amber-400 border border-amber-500/30'
                  : 'bg-muted/40 text-muted-foreground hover:bg-muted'
              }`}
              title="Real-time token streaming"
            >
              Streaming {streamingMode ? 'ON' : 'OFF'}
            </button>
          </div>

          <div className="flex items-center gap-2">
            {isStreaming ? (
              <button
                onClick={stopTurn}
                className="flex items-center gap-1.5 rounded-full bg-red-600 px-3 py-1.5 font-medium text-white shadow-md hover:bg-red-500 transition"
              >
                <Square className="h-3.5 w-3.5 fill-current" /> Stop
              </button>
            ) : (
              <button
                onClick={handleSend}
                disabled={!canSend}
                className="flex items-center gap-1.5 rounded-full bg-gradient-to-r from-amber-500 to-orange-500 px-3.5 py-1.5 font-medium text-white shadow-md hover:opacity-90 transition disabled:opacity-40 disabled:cursor-not-allowed"
              >
                <Send className="h-3.5 w-3.5" /> Send
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};
