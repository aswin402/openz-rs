import React, { useEffect, useMemo, useState } from 'react';
import {
  Activity,
  AlertCircle,
  ArrowLeft,
  Bot,
  BookOpen,
  Braces,
  Brain,
  Check,
  Code2,
  Cpu,
  Download,
  FileDiff as FileDiffIcon,
  Gauge,
  Image as ImageIcon,
  LayoutList,
  ListChecks,
  MessageSquare,
  Monitor,
  Moon,
  MousePointerClick,
  Palette,
  RotateCcw,
  ScrollText,
  Search,
  Send,
  Settings,
  ShieldCheck,
  Sparkles,
  Sun,
  Table2,
  Terminal,
  Upload,
  User,
  Wrench,
  X,
} from 'lucide-react';
import { useThemeStore, type Theme } from '../../store/useThemeStore';
import { useResolvedTheme } from '../../shared/lib/theme';
import {
  buildThemeSampleSvgDataUri,
  extractGatewayEndpointLabel,
  resolveModelContextWindow,
} from '../../shared/lib/runtimeMetrics';
import { useOpenZStore } from '../../store/useOpenZStore';
import {
  PREBUILT_THEMES,
  resolveActivePalette,
  useUiCustomizationStore,
  type CustomizeUiSectionId,
} from '../../store/useUiCustomizationStore';
import {
  AGENT_AVATAR_OPTIONS,
  AgentActivity,
  ApprovalCard,
  BeuiAgentAvatar,
  BeuiAlert,
  BeuiCitations,
  BeuiCodeBlock,
  BeuiCollapsible,
  BeuiContextUsage,
  BeuiDataTable,
  BeuiFileDiff,
  BeuiJsonViewer,
  BeuiMessageActions,
  BeuiModelSelector,
  BeuiPlanChecklist,
  BeuiPromptInput,
  BeuiPromptSuggestions,
  BeuiSubagentCard,
  BeuiTerminalOutput,
  BeuiUserAvatar,
  ImageGeneration,
  MessageScroller,
  ReasoningText,
  StreamingResponse,
  ToolApproval,
  ToolResult,
  USER_AVATAR_OPTIONS,
} from '../../components/beui';
import { OpenZFoxIcon } from '../../components/OpenZFoxIcon';
import { Badge } from '../../components/ui/badge';
import { Button } from '../../components/ui/button';
import { Input } from '../../components/ui/input';
import { Switch } from '../../components/ui/switch';
import { cn } from '../../lib/utils';

interface StudioMenuEntry {
  id: CustomizeUiSectionId;
  group: 'Appearance' | 'Composer & Chat' | 'Agent & Reasoning' | 'Code, Tools & Data';
  label: string;
  pkg: string;
  description: string;
  icon: React.ComponentType<{ className?: string }>;
}

const MENU_ITEMS: StudioMenuEntry[] = [
  {
    id: 'theme',
    group: 'Appearance',
    label: 'Global Theme',
    pkg: '@beui/theme',
    description: 'Select a dual-mode global color theme (with both Light & Dark palettes) across OpenZ.',
    icon: Palette,
  },
  {
    id: 'avatars',
    group: 'Appearance',
    label: 'Avatars & Fox Icon',
    pkg: '@beui/avatar',
    description: 'Choose from 13 Agent & Fox avatars (defaulting to our solid Fox silhouette) and 6 User avatars.',
    icon: User,
  },
  {
    id: 'prompt-input',
    group: 'Composer & Chat',
    label: 'Prompt Input',
    pkg: '@beui/prompt-input',
    description: 'Bottom chat composer surface and icon-only submit button variants.',
    icon: Send,
  },
  {
    id: 'model-selector',
    group: 'Composer & Chat',
    label: 'Model Selector',
    pkg: '@beui/model-selector',
    description: 'Searchable LLM provider & model command switcher inside the composer.',
    icon: Cpu,
  },
  {
    id: 'prompt-suggestions',
    group: 'Composer & Chat',
    label: 'Prompt Suggestions',
    pkg: '@beui/prompt-suggestions',
    description: 'Quick-start prompt cards and suggestion chips on empty chat sessions.',
    icon: Sparkles,
  },
  {
    id: 'context-usage',
    group: 'Composer & Chat',
    label: 'Context Meter',
    pkg: '@beui/context-usage',
    description: 'Live session token & Z-Context window usage indicator in the composer bar.',
    icon: Gauge,
  },
  {
    id: 'message-scroller',
    group: 'Composer & Chat',
    label: 'Message Scroller',
    pkg: '@beui/message-scroller',
    description: 'Auto-stick scroll behavior during token streaming and conversation track width.',
    icon: ScrollText,
  },
  {
    id: 'message-card',
    group: 'Composer & Chat',
    label: 'Message Bubble',
    pkg: '@beui/message',
    description: 'Two-ends message bubble surface aligned flush with both ends of the ChatBox.',
    icon: MessageSquare,
  },
  {
    id: 'message-actions',
    group: 'Composer & Chat',
    label: 'Message Actions',
    pkg: '@beui/message-actions',
    description: 'Hover toolbar for copying, regenerating, editing, quoting, and rating turns.',
    icon: MousePointerClick,
  },
  {
    id: 'streaming-response',
    group: 'Agent & Reasoning',
    label: 'Streaming Response',
    pkg: '@beui/streaming-response',
    description: 'Live token-streaming cursor and markdown response surface.',
    icon: Sparkles,
  },
  {
    id: 'reasoning-text',
    group: 'Agent & Reasoning',
    label: 'Reasoning Text',
    pkg: '@beui/reasoning-text',
    description: 'Expandable chain-of-thought reasoning trace and thinking state.',
    icon: Brain,
  },
  {
    id: 'agent-activity',
    group: 'Agent & Reasoning',
    label: 'Agent Activity',
    pkg: '@beui/agent-activity',
    description: 'Live multi-step agent timeline feed for tool calls, notices, and orchestration.',
    icon: Activity,
  },
  {
    id: 'plan-checklist',
    group: 'Agent & Reasoning',
    label: 'Plan Checklist',
    pkg: '@beui/plan-checklist',
    description: 'Step-by-step agent task checklist and completion progress bar.',
    icon: ListChecks,
  },
  {
    id: 'subagent-card',
    group: 'Agent & Reasoning',
    label: 'Subagent Card',
    pkg: '@beui/subagent-card',
    description: 'Delegated subagent & multi-agent workflow orchestration cards.',
    icon: Bot,
  },
  {
    id: 'code-block',
    group: 'Code, Tools & Data',
    label: 'Code Block',
    pkg: '@beui/code-block',
    description: 'Syntax-highlighted code blocks with line numbers, wrap toggle, download, and copy.',
    icon: Code2,
  },
  {
    id: 'file-diff',
    group: 'Code, Tools & Data',
    label: 'File Diff',
    pkg: '@beui/file-diff',
    description: 'Unified, split side-by-side, and compact hunk patch viewers.',
    icon: FileDiffIcon,
  },
  {
    id: 'terminal-output',
    group: 'Code, Tools & Data',
    label: 'Terminal Output',
    pkg: '@beui/terminal-output',
    description: 'Streaming shell & cargo command console output cards with exit status badges.',
    icon: Terminal,
  },
  {
    id: 'tool-result',
    group: 'Code, Tools & Data',
    label: 'Tool Result',
    pkg: '@beui/tool-result',
    description: 'Collapsible native tool execution cards with Result, Diff, Console, and JSON tabs.',
    icon: Wrench,
  },
  {
    id: 'tool-approval',
    group: 'Code, Tools & Data',
    label: 'Tool Approval',
    pkg: '@beui/tool-approval',
    description: 'SecurityGuard human-in-the-loop approval prompts before sensitive tool runs.',
    icon: ShieldCheck,
  },
  {
    id: 'image-generation',
    group: 'Code, Tools & Data',
    label: 'Image Generation',
    pkg: '@beui/image-generation',
    description: 'Rendered PNG/SVG media frames with prompt metadata, lightbox zoom, and download.',
    icon: ImageIcon,
  },
  {
    id: 'citations',
    group: 'Code, Tools & Data',
    label: 'Citations',
    pkg: '@beui/citations',
    description: 'Interactive web source badges, hover cards, expandable source decks, and footnotes.',
    icon: BookOpen,
  },
  {
    id: 'data-table',
    group: 'Code, Tools & Data',
    label: 'Data Table',
    pkg: '@beui/data-table',
    description: 'Sortable, filterable Markdown data tables with one-click TSV copy.',
    icon: Table2,
  },
  {
    id: 'json-viewer',
    group: 'Code, Tools & Data',
    label: 'JSON Viewer',
    pkg: '@beui/json-viewer',
    description: 'Interactive collapsible JSON tree inspector for tool arguments and structured outputs.',
    icon: Braces,
  },
  {
    id: 'alert-collapsible',
    group: 'Code, Tools & Data',
    label: 'Alert & Collapsible',
    pkg: '@beui/alert',
    description: 'Semantic status callouts and expandable disclosure containers.',
    icon: AlertCircle,
  },
];

const MENU_GROUPS: Array<StudioMenuEntry['group']> = [
  'Appearance',
  'Composer & Chat',
  'Agent & Reasoning',
  'Code, Tools & Data',
];

const SAMPLE_RUST_CODE = [
  'pub async fn run_turn(ctx: &mut TurnContext) -> Result<TurnOutcome> {',
  '    let guard = SecurityGuard::new(ctx.policy.clone());',
  '    let response = ctx.provider.chat(&ctx.messages).await?;',
  '    Ok(TurnOutcome::Completed(response))',
  '}',
].join('\n');

const SAMPLE_PATCH = [
  '--- a/src/agent/loop.rs',
  '+++ b/src/agent/loop.rs',
  '@@ -14,2 +14,3 @@',
  '-let streaming = false;',
  '+let streaming = true;',
  '+let ui = BeuiRegistry::default();',
].join('\n');

const SAMPLE_CITATIONS = [
  { n: 1, label: 'BEUI Agent Components', host: 'beui.dev', url: 'https://beui.dev' },
  { n: 2, label: 'shadcn/ui Documentation', host: 'ui.shadcn.com', url: 'https://ui.shadcn.com' },
  { n: 3, label: 'Tokio Async Runtime', host: 'tokio.rs', url: 'https://tokio.rs' },
];

interface VariantPreviewCardProps {
  title: string;
  description: string;
  selected: boolean;
  isDefault?: boolean;
  onSelect: () => void;
  children: React.ReactNode;
}

/**
 * shadcn/ui-style Component Variant Preview Card:
 * - Top: Variant title + Default / Selected badges
 * - Center: Live interactive @beui/* component preview stage
 * - Bottom ("in down"): Description + Select / Selected button
 */
const VariantPreviewCard: React.FC<VariantPreviewCardProps> = ({
  title,
  description,
  selected,
  isDefault = false,
  onSelect,
  children,
}) => {
  return (
    <div
      className={cn(
        'flex flex-col overflow-hidden rounded-xl border bg-card transition-colors',
        selected ? 'border-primary ring-1 ring-primary/35' : 'border-border/80 hover:border-border',
      )}
    >
      {/* Card Top Header */}
      <div className="flex items-center justify-between gap-2 border-b border-border/60 bg-muted/25 px-4 py-2.5">
        <div className="flex items-center gap-2">
          <span className="text-xs font-semibold text-foreground">{title}</span>
          {isDefault && (
            <Badge variant="outline" className="px-1.5 py-0 text-[10px]">
              Default
            </Badge>
          )}
        </div>
        {selected && (
          <Badge variant="success" className="gap-1 px-2 py-0 text-[10px]">
            <Check className="size-2.5" />
            <span>Active</span>
          </Badge>
        )}
      </div>

      {/* Live Component Preview Canvas */}
      <div className="flex min-h-[185px] flex-1 items-center justify-center bg-background/90 p-5">
        <div className="w-full max-w-2xl">{children}</div>
      </div>

      {/* Bottom Bar with Select Button ("in down give select button") */}
      <div className="flex items-center justify-between gap-3 border-t border-border/60 bg-card px-4 py-3">
        <p className="min-w-0 flex-1 text-xs text-muted-foreground">{description}</p>
        <Button
          type="button"
          size="sm"
          variant={selected ? 'secondary' : 'default'}
          onClick={onSelect}
          className={cn(
            'shrink-0 min-w-[96px]',
            selected && 'border border-primary/45 bg-primary/15 text-primary hover:bg-primary/20',
          )}
        >
          {selected ? (
            <>
              <Check className="size-3.5" />
              <span>Selected</span>
            </>
          ) : (
            <span>Select</span>
          )}
        </Button>
      </div>
    </div>
  );
};

export const UiStudioModal: React.FC = () => {
  const isOpen = useUiCustomizationStore((s) => s.isCustomizeOpen);
  const setIsOpen = useUiCustomizationStore((s) => s.setIsCustomizeOpen);
  const activeSection = useUiCustomizationStore((s) => s.activeCustomizeSection);
  const setActiveSection = useUiCustomizationStore((s) => s.setActiveCustomizeSection);
  const applyCustomization = useUiCustomizationStore((s) => s.applyCustomization);
  const resetAllCustomization = useUiCustomizationStore((s) => s.resetAllCustomization);
  const selectThemePreset = useUiCustomizationStore((s) => s.selectThemePreset);
  const exportBackupJson = useUiCustomizationStore((s) => s.exportBackupJson);
  const importBackupJson = useUiCustomizationStore((s) => s.importBackupJson);

  const state = useUiCustomizationStore();
  const currentThemeMode = useThemeStore((s) => s.theme);
  const resolvedMode = useResolvedTheme(currentThemeMode);
  const setThemeMode = useThemeStore((s) => s.setTheme);
  const setIsSettingsOpen = useOpenZStore((s) => s.setIsSettingsOpen);
  const activeModel = useOpenZStore((s) => s.activeModel);
  const activeProvider = useOpenZStore((s) => s.activeProvider);
  const settings = useOpenZStore((s) => s.settings);
  const wsUrl = useOpenZStore((s) => s.wsUrl);
  const runtimeInventory = useOpenZStore((s) => s.runtimeInventory);

  const [menuQuery, setMenuQuery] = useState('');
  const [demoPrompt, setDemoPrompt] = useState('');
  const [backupStatus, setBackupStatus] = useState<string | null>(null);
  const backupFileInputRef = React.useRef<HTMLInputElement>(null);

  const activePalette = useMemo(
    () => resolveActivePalette(state.activeThemeId, resolvedMode),
    [state.activeThemeId, resolvedMode],
  );
  const sampleSvgDataUri = useMemo(
    () => buildThemeSampleSvgDataUri(activePalette),
    [activePalette],
  );
  const nativeToolCount = runtimeInventory?.counts.tools ?? 260;
  const gatewayEndpoint = useMemo(() => extractGatewayEndpointLabel(wsUrl), [wsUrl]);
  const previewMaxTokens = useMemo(
    () => resolveModelContextWindow(activeModel, settings?.max_tokens),
    [activeModel, settings?.max_tokens],
  );
  const previewUsedTokens = Math.round(previewMaxTokens * 0.14);
  const previewModelLabel = activeModel || settings?.model || 'openz-default';

  useEffect(() => {
    if (!isOpen) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setIsOpen(false);
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [isOpen, setIsOpen]);

  const filteredMenu = useMemo(() => {
    const q = menuQuery.trim().toLowerCase();
    if (!q) return MENU_ITEMS;
    return MENU_ITEMS.filter(
      (item) =>
        item.label.toLowerCase().includes(q) ||
        item.pkg.toLowerCase().includes(q) ||
        item.group.toLowerCase().includes(q),
    );
  }, [menuQuery]);

  if (!isOpen) return null;

  const activeMeta =
    MENU_ITEMS.find((m) => m.id === activeSection) ?? MENU_ITEMS[0];
  const ActiveIcon = activeMeta.icon;

  const handleSelectMode = (mode: Theme) => {
    setThemeMode(mode);
  };

  const handleChooseGlobalTheme = (themeId: string, explicitMode?: 'dark' | 'light') => {
    const family = selectThemePreset(themeId);
    if (!family) return;
    if (explicitMode) {
      setThemeMode(explicitMode);
    }
  };

  const handleResetAll = () => {
    resetAllCustomization();
    setThemeMode('dark');
    setBackupStatus('Defaults restored & backed up');
  };

  const handleExportBackup = () => {
    const json = exportBackupJson({
      themeMode: currentThemeMode,
      gatewayDefaults: settings ? (settings as unknown as Record<string, unknown>) : null,
    });
    const blob = new Blob([json], { type: 'application/json;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = `openz-ui-backup-${new Date().toISOString().slice(0, 10)}.json`;
    document.body.appendChild(link);
    link.click();
    link.remove();
    URL.revokeObjectURL(url);
    setBackupStatus('Backup exported');
  };

  const handleImportBackupFile = (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    event.target.value = '';
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      const content = typeof reader.result === 'string' ? reader.result : '';
      const result = importBackupJson(content);
      if (!result.ok) {
        setBackupStatus(result.error || 'Invalid backup file');
        return;
      }
      if (result.themeMode) {
        setThemeMode(result.themeMode);
      }
      setBackupStatus('Backup restored & synced');
    };
    reader.readAsText(file);
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Customize UI"
      className="fixed inset-0 z-50 flex h-screen w-screen flex-col overflow-hidden bg-background text-foreground"
    >
      {/* Clean shadcn/ui Top Bar */}
      <header className="flex h-14 shrink-0 items-center justify-between gap-3 border-b border-border/70 bg-background px-4 md:px-6">
        <div className="flex min-w-0 items-center gap-3">
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => setIsOpen(false)}
          >
            <ArrowLeft className="size-3.5" />
            <span>Back to Chat</span>
          </Button>
          <div className="hidden h-4 w-px bg-border sm:block" />
          <div className="flex min-w-0 items-center gap-2">
            <OpenZFoxIcon size={18} variant="classic" />
            <span className="truncate text-sm font-bold tracking-tight">Customize UI</span>
            <Badge variant="secondary" className="hidden font-mono text-[10px] sm:inline-flex">
              @beui + shadcn/ui
            </Badge>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <input
            ref={backupFileInputRef}
            type="file"
            accept=".json,application/json"
            onChange={handleImportBackupFile}
            className="hidden"
          />
          <Badge variant="outline" className="hidden font-mono text-[10px] text-success lg:inline-flex">
            {backupStatus || 'Auto-saved & backed up'}
          </Badge>
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={handleExportBackup}
            title="Download a JSON backup of your selected theme and @beui component customizations"
            className="hidden md:inline-flex"
          >
            <Download className="size-3.5" />
            <span>Backup</span>
          </Button>
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => backupFileInputRef.current?.click()}
            title="Restore theme and UI customizations from a backup JSON file"
            className="hidden md:inline-flex"
          >
            <Upload className="size-3.5" />
            <span>Restore</span>
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={() => {
              setIsOpen(false);
              setIsSettingsOpen(true);
            }}
            className="hidden sm:inline-flex"
          >
            <Settings className="size-3.5" />
            <span>Gateway Settings</span>
          </Button>
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={handleResetAll}
            title="Reset all @beui components and global theme to default"
          >
            <RotateCcw className="size-3.5" />
            <span className="hidden sm:inline">Reset Defaults</span>
          </Button>
          <Button
            type="button"
            size="sm"
            onClick={() => setIsOpen(false)}
          >
            <Check className="size-3.5" />
            <span>Done</span>
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            onClick={() => setIsOpen(false)}
            aria-label="Close Customize UI"
          >
            <X className="size-4" />
          </Button>
        </div>
      </header>

      {/* Body: Left shadcn/ui Docs Component Sidebar + Right Component Options Preview Stage */}
      <div className="flex min-h-0 flex-1 flex-col overflow-hidden md:flex-row">
        {/* Left Sidebar Menu (shadcn/ui docs style) */}
        <aside className="flex w-full shrink-0 flex-col border-b border-border/65 bg-sidebar md:h-full md:w-64 md:border-b-0 md:border-r">
          <div className="border-b border-border/50 p-3">
            <div className="relative">
              <Search className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground" />
              <Input
                value={menuQuery}
                onChange={(e) => setMenuQuery(e.target.value)}
                placeholder="Filter components..."
                className="h-8 bg-background/60 pl-8 text-xs"
              />
            </div>
          </div>

          <nav className="flex gap-1 overflow-x-auto p-3 md:flex-1 md:flex-col md:overflow-x-hidden md:overflow-y-auto">
            {MENU_GROUPS.map((groupName) => {
              const items = filteredMenu.filter((m) => m.group === groupName);
              if (items.length === 0) return null;
              return (
                <div key={groupName} className="flex shrink-0 gap-1 md:mb-4 md:flex-col md:gap-0.5">
                  <div className="hidden px-2.5 pb-1.5 text-[10.5px] font-semibold uppercase tracking-wider text-muted-foreground/75 md:block">
                    {groupName}
                  </div>
                  <div className="flex gap-1 md:flex-col md:border-l md:border-border/60 md:pl-2">
                    {items.map((item) => {
                      const Icon = item.icon;
                      const active = item.id === activeSection;
                      return (
                        <button
                          key={item.id}
                          type="button"
                          onClick={() => setActiveSection(item.id)}
                          className={cn(
                            'flex shrink-0 items-center gap-2 rounded-md px-2.5 py-1.5 text-left text-xs transition-colors md:w-full',
                            active
                              ? 'bg-primary/15 font-semibold text-primary'
                              : 'text-muted-foreground hover:bg-muted/60 hover:text-foreground',
                          )}
                        >
                          <Icon
                            className={cn(
                              'size-3.5 shrink-0',
                              active ? 'text-primary' : 'text-muted-foreground/80',
                            )}
                          />
                          <span className="truncate">{item.label}</span>
                        </button>
                      );
                    })}
                  </div>
                </div>
              );
            })}
          </nav>
        </aside>

        {/* Right Main Preview Gallery */}
        <main className="min-w-0 flex-1 overflow-y-auto bg-background px-5 py-6 md:px-10 md:py-8">
          <div className="mx-auto max-w-5xl space-y-6 pb-14">
            {/* Component Page Heading */}
            <div className="flex flex-wrap items-start justify-between gap-3 border-b border-border/60 pb-5">
              <div className="space-y-1">
                <div className="flex items-center gap-2 text-xs text-muted-foreground">
                  <span>{activeMeta.group}</span>
                  <span>/</span>
                  <Badge variant="outline" className="font-mono text-[10.5px] text-primary">
                    {activeMeta.pkg}
                  </Badge>
                </div>
                <div className="flex items-center gap-2.5">
                  <ActiveIcon className="size-5 text-primary" />
                  <h1 className="text-xl font-bold tracking-tight text-foreground">
                    {activeMeta.label}
                  </h1>
                </div>
                <p className="text-xs text-muted-foreground md:text-sm">
                  {activeMeta.description}
                </p>
              </div>
            </div>

            {/* 1. GLOBAL THEME (Global Theme Color Only) */}
            {activeSection === 'theme' && (
              <div className="space-y-6">
                <div className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-border/80 bg-card p-4">
                  <div>
                    <div className="text-xs font-semibold text-foreground">Appearance Mode</div>
                    <p className="text-xs text-muted-foreground">
                      Global light, dark, or system-synced color mode across OpenZ
                    </p>
                  </div>
                  <div className="inline-flex rounded-lg border border-border bg-muted/40 p-0.5 text-xs">
                    {(
                      [
                        { id: 'dark', label: 'Dark', icon: Moon },
                        { id: 'light', label: 'Light', icon: Sun },
                        { id: 'system', label: 'System', icon: Monitor },
                      ] as const
                    ).map((m) => {
                      const Icon = m.icon;
                      const active = currentThemeMode === m.id;
                      return (
                        <button
                          key={m.id}
                          type="button"
                          onClick={() => handleSelectMode(m.id)}
                          className={cn(
                            'flex items-center gap-1.5 rounded-md px-3 py-1.5 font-medium transition-colors',
                            active
                              ? 'bg-card text-foreground shadow-2xs'
                              : 'text-muted-foreground hover:text-foreground',
                          )}
                        >
                          <Icon className="size-3.5" />
                          <span>{m.label}</span>
                        </button>
                      );
                    })}
                  </div>
                </div>

                <div className="grid grid-cols-1 gap-5 sm:grid-cols-2 lg:grid-cols-3">
                  {PREBUILT_THEMES.map((family, idx) => {
                    const selected = state.activeThemeId === family.id;
                    const activeSub = resolvedMode === 'light' ? family.light : family.dark;
                    return (
                      <VariantPreviewCard
                        key={family.id}
                        title={family.name}
                        description={`${family.description} · Dark ${family.dark.primaryHex} / Light ${family.light.primaryHex}`}
                        selected={selected}
                        isDefault={idx === 0}
                        onSelect={() => handleChooseGlobalTheme(family.id)}
                      >
                        <div className="space-y-2.5">
                          {/* Dual-Mode Interactive Swatches (Dark + Light for EVERY theme) */}
                          <div className="grid grid-cols-2 gap-2">
                            {(['dark', 'light'] as const).map((subMode) => {
                              const sub = family[subMode];
                              const isActiveSub = selected && resolvedMode === subMode;
                              return (
                                <button
                                  key={subMode}
                                  type="button"
                                  onClick={() => handleChooseGlobalTheme(family.id, subMode)}
                                  className="flex flex-col overflow-hidden rounded-xl border p-2.5 text-left text-xs shadow-2xs transition-transform hover:scale-[1.01]"
                                  style={{
                                    backgroundColor: sub.backgroundHex,
                                    borderColor: isActiveSub ? sub.primaryHex : sub.borderHex,
                                    color: sub.foregroundHex,
                                  }}
                                >
                                  <div className="flex w-full items-center justify-between pb-2">
                                    <div className="flex items-center gap-1">
                                      <OpenZFoxIcon
                                        size={14}
                                        variant="classic"
                                        tintColor={sub.primaryHex}
                                        accentColor={sub.accentHex}
                                      />
                                      <span className="text-[10.5px] font-bold">OpenZ</span>
                                    </div>
                                    <span
                                      className="rounded-full px-1.5 py-0.5 font-mono text-[9px] uppercase"
                                      style={{
                                        backgroundColor: sub.mutedHex,
                                        color: sub.mutedForegroundHex,
                                      }}
                                    >
                                      {subMode}
                                    </span>
                                  </div>
                                  <div
                                    className="w-full rounded-lg border p-2 text-[10px]"
                                    style={{
                                      backgroundColor: sub.cardHex,
                                      borderColor: sub.borderHex,
                                    }}
                                  >
                                    <div className="truncate font-medium">
                                      {subMode === 'dark' ? 'Dark Mode' : 'Light Mode'}
                                    </div>
                                    <div className="mt-1.5 flex items-center justify-between gap-1">
                                      <span
                                        className="font-mono text-[9px]"
                                        style={{ color: sub.mutedForegroundHex }}
                                      >
                                        {sub.primaryHex}
                                      </span>
                                      <span
                                        className="rounded px-1.5 py-0.5 text-[9px] font-semibold"
                                        style={{
                                          backgroundColor: sub.primaryHex,
                                          color: sub.primaryForegroundHex,
                                        }}
                                      >
                                        Aa
                                      </span>
                                    </div>
                                  </div>
                                </button>
                              );
                            })}
                          </div>

                          <div className="flex items-center justify-between px-0.5 text-[10.5px] text-muted-foreground">
                            <span>Supports Light &amp; Dark</span>
                            <span className="font-mono">{activeSub.primaryHex}</span>
                          </div>
                        </div>
                      </VariantPreviewCard>
                    );
                  })}
                </div>
              </div>
            )}

            {/* 2. PROMPT INPUT (@beui/prompt-input) */}
            {activeSection === 'prompt-input' && (
              <div className="space-y-8">
                <div className="grid grid-cols-1 gap-6">
                  {(
                    [
                      {
                        id: 'elevated',
                        title: 'Elevated Card Composer',
                        desc: 'Crisp bordered card surface with integrated single-row @beui toolbar.',
                        isDefault: true,
                      },
                      {
                        id: 'split',
                        title: 'Split Footer Composer',
                        desc: 'Distinct input canvas above a tinted footer toolbar strip.',
                        isDefault: false,
                      },
                      {
                        id: 'minimal',
                        title: 'Minimal Compact Composer',
                        desc: 'Low-profile compact bar with reduced vertical padding.',
                        isDefault: false,
                      },
                    ] as const
                  ).map((opt) => (
                    <VariantPreviewCard
                      key={opt.id}
                      title={opt.title}
                      description={opt.desc}
                      selected={state.promptInputStyle === opt.id}
                      isDefault={opt.isDefault}
                      onSelect={() => applyCustomization({ promptInputStyle: opt.id })}
                    >
                      <BeuiPromptInput
                        value={demoPrompt}
                        onChange={(e) => setDemoPrompt(e.target.value)}
                        onSubmit={() => setDemoPrompt('')}
                        canSend={demoPrompt.trim().length > 0 || true}
                        variant={opt.id}
                        sendButtonVariant={state.sendButtonStyle}
                        onAttachClick={() => {}}
                        leftToolbar={
                          <Badge variant="secondary" className="font-mono text-[10px]">
                            {previewModelLabel}
                          </Badge>
                        }
                      />
                    </VariantPreviewCard>
                  ))}
                </div>

                <div>
                  <h2 className="mb-3 text-sm font-semibold text-foreground">
                    Icon-Only Send Button Style
                  </h2>
                  <div className="grid grid-cols-1 gap-5 md:grid-cols-3">
                    {(
                      [
                        {
                          id: 'icon-round',
                          title: 'Round Paper Plane',
                          desc: 'Circular icon-only Send button without text.',
                          isDefault: true,
                        },
                        {
                          id: 'arrow-round',
                          title: 'Round Up-Arrow',
                          desc: 'Circular upward arrow submit icon.',
                          isDefault: false,
                        },
                        {
                          id: 'icon-square',
                          title: 'Rounded Square Icon',
                          desc: 'Compact squircle icon-only Send button.',
                          isDefault: false,
                        },
                      ] as const
                    ).map((opt) => (
                      <VariantPreviewCard
                        key={opt.id}
                        title={opt.title}
                        description={opt.desc}
                        selected={state.sendButtonStyle === opt.id}
                        isDefault={opt.isDefault}
                        onSelect={() => applyCustomization({ sendButtonStyle: opt.id })}
                      >
                        <div className="flex justify-center">
                          <BeuiPromptInput
                            value="Run focused check"
                            onChange={() => {}}
                            onSubmit={() => {}}
                            canSend
                            variant="minimal"
                            sendButtonVariant={opt.id}
                          />
                        </div>
                      </VariantPreviewCard>
                    ))}
                  </div>
                </div>
              </div>
            )}

            {/* 3. MODEL SELECTOR (@beui/model-selector) */}
            {activeSection === 'model-selector' && (
              <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
                {(
                  [
                    {
                      id: 'command',
                      title: 'Command Trigger',
                      desc: 'CPU icon + model name + searchable command popover.',
                      isDefault: true,
                    },
                    {
                      id: 'provider-badge',
                      title: 'Provider Badged Trigger',
                      desc: 'Highlights active provider pill next to the model name.',
                      isDefault: false,
                    },
                    {
                      id: 'compact-pill',
                      title: 'Minimal Pill Trigger',
                      desc: 'Rounded pill selector for ultra-clean composer bars.',
                      isDefault: false,
                    },
                  ] as const
                ).map((opt) => (
                  <VariantPreviewCard
                    key={opt.id}
                    title={opt.title}
                    description={opt.desc}
                    selected={state.modelSelectorStyle === opt.id}
                    isDefault={opt.isDefault}
                    onSelect={() => applyCustomization({ modelSelectorStyle: opt.id })}
                  >
                    <div className="flex justify-center py-4">
                      <BeuiModelSelector
                        activeModel={previewModelLabel}
                        activeProvider={activeProvider || 'anthropic'}
                        variant={opt.id}
                        groups={[
                          {
                            name: 'anthropic',
                            display: 'Anthropic',
                            models: ['claude-sonnet-4-20250514', 'claude-3-7-sonnet'],
                          },
                          {
                            name: 'openai',
                            display: 'OpenAI',
                            models: ['gpt-4.1', 'o4-mini'],
                          },
                        ]}
                        onSelectModel={() => {}}
                      />
                    </div>
                  </VariantPreviewCard>
                ))}
              </div>
            )}

            {/* 4. PROMPT SUGGESTIONS (@beui/prompt-suggestions) */}
            {activeSection === 'prompt-suggestions' && (
              <div className="grid grid-cols-1 gap-6">
                {(
                  [
                    {
                      id: 'bento',
                      title: '2-Column Bento Cards',
                      desc: 'Structured starter cards aligned flush with both ends of the ChatBox.',
                      isDefault: true,
                    },
                    {
                      id: 'pills',
                      title: 'Compact Pill Chips',
                      desc: 'Minimalist horizontal suggestion chips.',
                      isDefault: false,
                    },
                    {
                      id: 'list',
                      title: 'Stacked Action List',
                      desc: 'Clean single-column divided command list.',
                      isDefault: false,
                    },
                  ] as const
                ).map((opt) => (
                  <VariantPreviewCard
                    key={opt.id}
                    title={opt.title}
                    description={opt.desc}
                    selected={state.promptSuggestionsStyle === opt.id}
                    isDefault={opt.isDefault}
                    onSelect={() => applyCustomization({ promptSuggestionsStyle: opt.id })}
                  >
                    <BeuiPromptSuggestions
                      variant={opt.id}
                      onSelect={() => {}}
                      items={[
                        {
                          id: '1',
                          title: 'Analyze Codebase',
                          description: 'AST structural search + focused check',
                          prompt: 'Analyze',
                          icon: Code2,
                        },
                        {
                          id: '2',
                          title: 'Web Research',
                          description: 'Multi-engine search with @beui/citations',
                          prompt: 'Search',
                          icon: Search,
                        },
                      ]}
                    />
                  </VariantPreviewCard>
                ))}
              </div>
            )}

            {/* 5. CONTEXT METER (@beui/context-usage) */}
            {activeSection === 'context-usage' && (
              <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
                {(
                  [
                    {
                      id: 'ring-pill',
                      title: 'Radial Ring Pill',
                      desc: 'Compact circular SVG gauge with token count.',
                      isDefault: true,
                    },
                    {
                      id: 'bar',
                      title: 'Progress Bar Meter',
                      desc: 'Horizontal fill bar with compact token badge.',
                      isDefault: false,
                    },
                    {
                      id: 'hidden',
                      title: 'Hidden',
                      desc: 'Hide the token meter from the composer toolbar.',
                      isDefault: false,
                    },
                  ] as const
                ).map((opt) => (
                  <VariantPreviewCard
                    key={opt.id}
                    title={opt.title}
                    description={opt.desc}
                    selected={state.contextUsageStyle === opt.id}
                    isDefault={opt.isDefault}
                    onSelect={() => applyCustomization({ contextUsageStyle: opt.id })}
                  >
                    <div className="flex justify-center py-4">
                      {opt.id === 'hidden' ? (
                        <span className="text-xs text-muted-foreground">
                          Context meter hidden in composer
                        </span>
                      ) : (
                        <BeuiContextUsage
                          usedTokens={previewUsedTokens}
                          maxTokens={previewMaxTokens}
                          messageCount={12}
                          variant={opt.id}
                        />
                      )}
                    </div>
                  </VariantPreviewCard>
                ))}
              </div>
            )}

            {/* 6. MESSAGE SCROLLER (@beui/message-scroller) */}
            {activeSection === 'message-scroller' && (
              <div className="space-y-8">
                <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
                  {(
                    [
                      {
                        id: 'beui-smart',
                        title: 'BEUI Smart Stick',
                        desc: 'Auto-scrolls at bottom; pauses cleanly if you scroll up to read.',
                        isDefault: true,
                      },
                      {
                        id: 'always-follow',
                        title: 'Always Follow Stream',
                        desc: 'Continuously locks onto every incoming streamed token.',
                        isDefault: false,
                      },
                      {
                        id: 'manual',
                        title: 'Manual Reader',
                        desc: 'Never jumps automatically; shows floating Jump-to-latest pill.',
                        isDefault: false,
                      },
                    ] as const
                  ).map((opt) => (
                    <VariantPreviewCard
                      key={opt.id}
                      title={opt.title}
                      description={opt.desc}
                      selected={state.scrollerBehavior === opt.id}
                      isDefault={opt.isDefault}
                      onSelect={() => applyCustomization({ scrollerBehavior: opt.id })}
                    >
                      <div className="h-36 overflow-hidden rounded-xl border border-border/75 bg-card/60">
                        <MessageScroller
                          itemCount={3}
                          isStreaming={opt.id !== 'manual'}
                          behaviorMode={opt.id}
                          contentClassName="p-3 space-y-2 text-xs"
                        >
                          <div className="rounded-lg bg-muted/50 px-3 py-1.5">
                            User prompt aligned to right edge
                          </div>
                          <div className="rounded-lg border border-border/60 bg-background px-3 py-1.5">
                            Streaming @beui/message-scroller…
                          </div>
                        </MessageScroller>
                      </div>
                    </VariantPreviewCard>
                  ))}
                </div>

                <div>
                  <h2 className="mb-3 text-sm font-semibold text-foreground">
                    Conversation &amp; ChatBox Track Width
                  </h2>
                  <div className="grid grid-cols-1 gap-5 md:grid-cols-3">
                    {(
                      [
                        {
                          id: 'standard',
                          title: 'Balanced (Default)',
                          desc: 'Optimal 820px reading column with outer Fox & User gutters.',
                          isDefault: true,
                        },
                        {
                          id: 'wide',
                          title: 'Wide Studio',
                          desc: 'Expanded 1080px track ideal for split diffs & wide tables.',
                          isDefault: false,
                        },
                        {
                          id: 'full',
                          title: 'Full Edge-to-Edge',
                          desc: 'Spans full available viewport width.',
                          isDefault: false,
                        },
                      ] as const
                    ).map((opt) => (
                      <VariantPreviewCard
                        key={opt.id}
                        title={opt.title}
                        description={opt.desc}
                        selected={state.chatWidth === opt.id}
                        isDefault={opt.isDefault}
                        onSelect={() => applyCustomization({ chatWidth: opt.id })}
                      >
                        <div className="flex items-center justify-center py-4">
                          <div
                            className={cn(
                              'rounded-lg border border-primary/40 bg-primary/10 py-3 text-center font-mono text-xs text-primary',
                              opt.id === 'standard'
                                ? 'w-3/5'
                                : opt.id === 'wide'
                                  ? 'w-4/5'
                                  : 'w-full',
                            )}
                          >
                            <LayoutList className="mx-auto mb-1 size-4" />
                            <span>{opt.title}</span>
                          </div>
                        </div>
                      </VariantPreviewCard>
                    ))}
                  </div>
                </div>
              </div>
            )}

            {/* AVATARS & FOX ICON (@beui/avatar) */}
            {activeSection === 'avatars' && (
              <div className="space-y-8">
                <div>
                  <h2 className="mb-1 text-sm font-semibold text-foreground">
                    Agent &amp; Fox Avatars (13 Options)
                  </h2>
                  <p className="mb-4 text-xs text-muted-foreground">
                    Includes our signature solid OpenZ Fox silhouette, sculpted Fox avatars, and minimal BEUI agent emblems.
                  </p>
                  <div className="grid grid-cols-1 gap-5 sm:grid-cols-2 lg:grid-cols-3">
                    {AGENT_AVATAR_OPTIONS.map((opt) => (
                      <VariantPreviewCard
                        key={opt.id}
                        title={opt.title}
                        description={opt.description}
                        selected={state.agentAvatar === opt.id}
                        isDefault={Boolean(opt.isDefault)}
                        onSelect={() => applyCustomization({ agentAvatar: opt.id })}
                      >
                        <div className="flex flex-col items-center gap-3 py-2">
                          {opt.id === 'none' ? (
                            <div className="flex h-11 items-center justify-center rounded-full border border-dashed border-border px-4 text-xs text-muted-foreground">
                              Hidden Avatar
                            </div>
                          ) : (
                            <BeuiAgentAvatar variant={opt.id} size="lg" />
                          )}
                          <div className="flex w-full items-center gap-2 rounded-xl border border-border/70 bg-card px-3 py-2 text-xs">
                            <BeuiAgentAvatar variant={opt.id} size="sm" />
                            <span className="truncate text-foreground/90">
                              OpenZ Agent ready.
                            </span>
                          </div>
                        </div>
                      </VariantPreviewCard>
                    ))}
                  </div>
                </div>

                <div>
                  <h2 className="mb-1 text-sm font-semibold text-foreground">
                    User (&ldquo;Ours&rdquo;) Outer-Right Avatars (6 Options)
                  </h2>
                  <p className="mb-4 text-xs text-muted-foreground">
                    Displayed in the outer-right gutter next to your prompts.
                  </p>
                  <div className="grid grid-cols-1 gap-5 sm:grid-cols-2 lg:grid-cols-3">
                    {USER_AVATAR_OPTIONS.map((opt) => (
                      <VariantPreviewCard
                        key={opt.id}
                        title={opt.title}
                        description={opt.description}
                        selected={state.userAvatar === opt.id}
                        isDefault={Boolean(opt.isDefault)}
                        onSelect={() => applyCustomization({ userAvatar: opt.id })}
                      >
                        <div className="flex flex-col items-center gap-3 py-2">
                          {opt.id === 'none' ? (
                            <div className="flex h-11 items-center justify-center rounded-full border border-dashed border-border px-4 text-xs text-muted-foreground">
                              Hidden User Avatar
                            </div>
                          ) : (
                            <BeuiUserAvatar variant={opt.id} size="lg" />
                          )}
                          <div className="flex w-full items-center justify-end gap-2">
                            <span className="rounded-2xl bg-primary px-3 py-1.5 text-xs text-primary-foreground">
                              Run check
                            </span>
                            <BeuiUserAvatar variant={opt.id} size="sm" />
                          </div>
                        </div>
                      </VariantPreviewCard>
                    ))}
                  </div>
                </div>
              </div>
            )}

            {/* 7. MESSAGE BUBBLE */}
            {activeSection === 'message-card' && (
              <div className="space-y-6">
                <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
                  {(
                    [
                      {
                        id: 'beui-card',
                        title: 'BEUI Card Bubble',
                        desc: 'Crisp bordered assistant card & primary user pill aligned to ChatBox ends.',
                        isDefault: true,
                      },
                      {
                        id: 'beui-full',
                        title: 'BEUI Full-Width Span',
                        desc: 'Assistant card always spans 100% between left and right ChatBox ends.',
                        isDefault: false,
                      },
                      {
                        id: 'editorial',
                        title: 'Editorial Borderless',
                        desc: 'Clean unboxed assistant prose with subtle user pill.',
                        isDefault: false,
                      },
                    ] as const
                  ).map((opt) => (
                    <VariantPreviewCard
                      key={opt.id}
                      title={opt.title}
                      description={opt.desc}
                      selected={state.bubbleStyle === opt.id}
                      isDefault={opt.isDefault}
                      onSelect={() => applyCustomization({ bubbleStyle: opt.id })}
                    >
                      <div className="space-y-2.5 text-xs">
                        <div className="flex items-center justify-end gap-2">
                          <div className="rounded-2xl bg-primary px-3 py-1.5 text-primary-foreground">
                            Check status
                          </div>
                          <BeuiUserAvatar variant={state.userAvatar} size="sm" />
                        </div>
                        <div className="flex items-start gap-2">
                          <BeuiAgentAvatar variant={state.agentAvatar} size="sm" />
                          <div
                            className={cn(
                              'py-2 text-foreground',
                              opt.id === 'editorial'
                                ? 'w-full px-1'
                                : opt.id === 'beui-full'
                                  ? 'w-full rounded-xl border border-border bg-card px-3'
                                  : 'rounded-xl border border-border bg-card px-3',
                            )}
                          >
                            All {nativeToolCount} native tools ready.
                          </div>
                        </div>
                      </div>
                    </VariantPreviewCard>
                  ))}
                </div>
              </div>
            )}

            {/* 8. MESSAGE ACTIONS (@beui/message-actions) */}
            {activeSection === 'message-actions' && (
              <div className="space-y-6">
                <div className="flex items-center justify-between rounded-xl border border-border/80 bg-card p-4">
                  <div>
                    <div className="text-xs font-semibold text-foreground">
                      Show Hover Message Actions
                    </div>
                    <p className="text-xs text-muted-foreground">
                      Display Copy, Regenerate, Edit, Quote Reply, and Feedback controls under messages
                    </p>
                  </div>
                  <Switch
                    checked={state.showMessageActions}
                    onCheckedChange={(checked) =>
                      applyCustomization({ showMessageActions: checked })
                    }
                  />
                </div>

                <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
                  {(
                    [
                      {
                        id: 'floating',
                        title: 'Floating Pill Bar',
                        desc: 'Bordered @beui toolbar card with full action set.',
                        isDefault: true,
                      },
                      {
                        id: 'ghost',
                        title: 'Minimal Ghost Row',
                        desc: 'Borderless subtle icon row beneath turns.',
                        isDefault: false,
                      },
                      {
                        id: 'compact',
                        title: 'Compact Essential',
                        desc: 'Condensed Copy, Regenerate, and Quote Reply bar.',
                        isDefault: false,
                      },
                    ] as const
                  ).map((opt) => (
                    <VariantPreviewCard
                      key={opt.id}
                      title={opt.title}
                      description={opt.desc}
                      selected={state.messageActionsStyle === opt.id}
                      isDefault={opt.isDefault}
                      onSelect={() => applyCustomization({ messageActionsStyle: opt.id })}
                    >
                      <div className="flex justify-center py-4">
                        <BeuiMessageActions
                          text="Sample assistant response"
                          variant={opt.id}
                          onRegenerate={() => {}}
                          onReply={() => {}}
                          onFork={() => {}}
                        />
                      </div>
                    </VariantPreviewCard>
                  ))}
                </div>
              </div>
            )}

            {/* 9. STREAMING RESPONSE (@beui/streaming-response) */}
            {activeSection === 'streaming-response' && (
              <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
                {(
                  [
                    {
                      id: 'beui-pulse',
                      title: 'BEUI Pulse Dot',
                      desc: 'Pulsing primary dot tracking the live token stream.',
                      isDefault: true,
                    },
                    {
                      id: 'smooth-caret',
                      title: 'Smooth Caret Bar',
                      desc: 'Crisp vertical caret bar at the active token edge.',
                      isDefault: false,
                    },
                    {
                      id: 'block-cursor',
                      title: 'Terminal Block Cursor',
                      desc: 'Solid blinking terminal block cursor while tokens arrive.',
                      isDefault: false,
                    },
                    {
                      id: 'none',
                      title: 'Clean Instant',
                      desc: 'Stream text cleanly with no trailing cursor indicator.',
                      isDefault: false,
                    },
                  ] as const
                ).map((opt) => (
                  <VariantPreviewCard
                    key={opt.id}
                    title={opt.title}
                    description={opt.desc}
                    selected={state.streamingAnimation === opt.id}
                    isDefault={opt.isDefault}
                    onSelect={() => applyCustomization({ streamingAnimation: opt.id })}
                  >
                    <div className="rounded-xl border border-border/75 bg-card p-4 text-xs">
                      <StreamingResponse isStreaming cursorStyle={opt.id}>
                        <span>
                          Streaming async response tokens through OpenZ WebSocket gateway
                        </span>
                      </StreamingResponse>
                    </div>
                  </VariantPreviewCard>
                ))}
              </div>
            )}

            {/* 10. REASONING TEXT (@beui/reasoning-text) */}
            {activeSection === 'reasoning-text' && (
              <div className="grid grid-cols-1 gap-6">
                {(
                  [
                    {
                      id: 'card',
                      title: 'BEUI Collapsible Reasoning Card',
                      desc: 'Expandable chain-of-thought block with step count badge and monospace trace.',
                      isDefault: true,
                    },
                    {
                      id: 'timeline',
                      title: 'BEUI Numbered Reasoning Timeline',
                      desc: 'Clean numbered reasoning steps along a left accent rail.',
                      isDefault: false,
                    },
                    {
                      id: 'compact',
                      title: 'BEUI Compact Status Pill',
                      desc: 'Single-line live thinking summary pill without expanding height.',
                      isDefault: false,
                    },
                  ] as const
                ).map((opt) => (
                  <VariantPreviewCard
                    key={opt.id}
                    title={opt.title}
                    description={opt.desc}
                    selected={state.thinkingAnimation === opt.id}
                    isDefault={opt.isDefault}
                    onSelect={() => applyCustomization({ thinkingAnimation: opt.id })}
                  >
                    <ReasoningText
                      variant={opt.id}
                      defaultOpen
                      text={
                        '1. Inspecting Cargo.toml & WebUI component tree\n2. Verifying zero-regex guard invariants\n3. Preparing concise response'
                      }
                    />
                  </VariantPreviewCard>
                ))}
              </div>
            )}

            {/* 11. AGENT ACTIVITY (@beui/agent-activity) */}
            {activeSection === 'agent-activity' && (
              <div className="grid grid-cols-1 gap-6">
                {(
                  [
                    {
                      id: 'beui-timeline',
                      title: 'BEUI Smart Inline + Panel Timeline',
                      desc: 'Shows collapsible inline step timeline on multi-tool turns plus right-hand panel.',
                      isDefault: true,
                    },
                    {
                      id: 'always-inline',
                      title: 'Always Inline Timeline',
                      desc: 'Embeds @beui/agent-activity inside every assistant turn with tool or notice events.',
                      isDefault: false,
                    },
                    {
                      id: 'panel-only',
                      title: 'Right Panel Only',
                      desc: 'Keeps chat bubbles minimal and displays activity exclusively in the right drawer.',
                      isDefault: false,
                    },
                  ] as const
                ).map((opt) => (
                  <VariantPreviewCard
                    key={opt.id}
                    title={opt.title}
                    description={opt.desc}
                    selected={state.activityStyle === opt.id}
                    isDefault={opt.isDefault}
                    onSelect={() => applyCustomization({ activityStyle: opt.id })}
                  >
                    <AgentActivity
                      isActive={opt.id !== 'panel-only'}
                      mode={opt.id === 'always-inline' ? 'timeline' : 'inline-collapsible'}
                      steps={[
                        {
                          id: 'a1',
                          kind: 'reasoning',
                          title: 'Planned verification steps',
                          status: 'info',
                        },
                        {
                          id: 'a2',
                          kind: 'tool',
                          title: 'ast_grep',
                          detail: 'Scanned 24 Rust modules',
                          status: 'success',
                          durationLabel: '95ms',
                        },
                      ]}
                    />
                  </VariantPreviewCard>
                ))}
              </div>
            )}

            {/* 12. PLAN CHECKLIST (@beui/plan-checklist) */}
            {activeSection === 'plan-checklist' && (
              <div className="grid grid-cols-1 gap-6">
                {(
                  [
                    {
                      id: 'card',
                      title: 'BEUI Progress Checklist Card',
                      desc: 'Full task checklist card with animated progress bar and completion ratio.',
                      isDefault: true,
                    },
                    {
                      id: 'compact-steps',
                      title: 'BEUI Compact Step Pills',
                      desc: 'Horizontal inline step chips for dense multi-tool turns.',
                      isDefault: false,
                    },
                    {
                      id: 'minimal',
                      title: 'BEUI Minimal Rail Checklist',
                      desc: 'Borderless left-accent checklist integrated smoothly into prose.',
                      isDefault: false,
                    },
                  ] as const
                ).map((opt) => (
                  <VariantPreviewCard
                    key={opt.id}
                    title={opt.title}
                    description={opt.desc}
                    selected={state.planChecklistStyle === opt.id}
                    isDefault={opt.isDefault}
                    onSelect={() => applyCustomization({ planChecklistStyle: opt.id })}
                  >
                    <BeuiPlanChecklist
                      variant={opt.id}
                      steps={[
                        { id: 's1', title: 'Read workspace manifests', status: 'completed' },
                        { id: 's2', title: 'Compile @beui components', status: 'completed' },
                        { id: 's3', title: 'Run test-guard verification', status: 'running' },
                      ]}
                    />
                  </VariantPreviewCard>
                ))}
              </div>
            )}

            {/* 13. SUBAGENT CARD (@beui/subagent-card) */}
            {activeSection === 'subagent-card' && (
              <div className="grid grid-cols-1 gap-6">
                {(
                  [
                    {
                      id: 'card',
                      title: 'BEUI Delegation Card',
                      desc: 'Rich subagent card displaying profile badge, goal, step status, and summary.',
                      isDefault: true,
                    },
                    {
                      id: 'tree',
                      title: 'BEUI Branch Tree Card',
                      desc: 'Indented vertical branch rail highlighting nested multi-agent workflow steps.',
                      isDefault: false,
                    },
                    {
                      id: 'compact',
                      title: 'BEUI Compact Subagent Row',
                      desc: 'Single-line badge row showing delegated profile, goal, and status icon.',
                      isDefault: false,
                    },
                  ] as const
                ).map((opt) => (
                  <VariantPreviewCard
                    key={opt.id}
                    title={opt.title}
                    description={opt.desc}
                    selected={state.subagentCardStyle === opt.id}
                    isDefault={opt.isDefault}
                    onSelect={() => applyCustomization({ subagentCardStyle: opt.id })}
                  >
                    <BeuiSubagentCard
                      agentName="researcher"
                      mode="parallel"
                      goal="Audit async WebSocket streaming & session persistence"
                      status="success"
                      durationLabel="1.4s"
                      variant={opt.id}
                      steps={[
                        {
                          id: 'protocol-audit',
                          agent: 'reviewer',
                          status: 'success',
                          output: 'Verified typed WS frame serialization.',
                        },
                      ]}
                    />
                  </VariantPreviewCard>
                ))}
              </div>
            )}

            {/* 14. CODE BLOCK (@beui/code-block) */}
            {activeSection === 'code-block' && (
              <div className="grid grid-cols-1 gap-6">
                {(
                  [
                    {
                      id: 'framed',
                      title: 'BEUI Framed Studio Code Block',
                      desc: 'Full header with file icon, language badge, line count, wrap toggle, download, and copy.',
                      isDefault: true,
                    },
                    {
                      id: 'minimal',
                      title: 'BEUI Minimal Header Code Block',
                      desc: 'Clean understated header with line numbers, wrap toggle, and copy button.',
                      isDefault: false,
                    },
                    {
                      id: 'compact',
                      title: 'BEUI Compact Snippet',
                      desc: 'Dense gutterless code snippet for crisp inline technical answers.',
                      isDefault: false,
                    },
                  ] as const
                ).map((opt) => (
                  <VariantPreviewCard
                    key={opt.id}
                    title={opt.title}
                    description={opt.desc}
                    selected={state.codeStyle === opt.id}
                    isDefault={opt.isDefault}
                    onSelect={() => applyCustomization({ codeStyle: opt.id })}
                  >
                    <BeuiCodeBlock
                      language="rust"
                      filename="src/agent/turn.rs"
                      code={SAMPLE_RUST_CODE}
                      variant={opt.id}
                      className="my-0"
                    />
                  </VariantPreviewCard>
                ))}
              </div>
            )}

            {/* 15. FILE DIFF (@beui/file-diff) */}
            {activeSection === 'file-diff' && (
              <div className="grid grid-cols-1 gap-6">
                {(
                  [
                    {
                      id: 'unified',
                      title: 'BEUI Unified Patch View',
                      desc: 'Single-column +/- diff with dual old/new line numbers and syntax highlighting.',
                      isDefault: true,
                    },
                    {
                      id: 'split',
                      title: 'BEUI Split Side-by-Side Diff',
                      desc: 'Two-column before/after comparison layout for code reviews.',
                      isDefault: false,
                    },
                    {
                      id: 'compact',
                      title: 'BEUI Changed Hunks Only',
                      desc: 'Filters out unchanged context rows to focus strictly on +/- edits.',
                      isDefault: false,
                    },
                  ] as const
                ).map((opt) => (
                  <VariantPreviewCard
                    key={opt.id}
                    title={opt.title}
                    description={opt.desc}
                    selected={state.diffLayout === opt.id}
                    isDefault={opt.isDefault}
                    onSelect={() => applyCustomization({ diffLayout: opt.id })}
                  >
                    <BeuiFileDiff
                      file="src/agent/loop.rs"
                      patch={SAMPLE_PATCH}
                      defaultView={opt.id}
                      className="my-0"
                    />
                  </VariantPreviewCard>
                ))}
              </div>
            )}

            {/* 16. TERMINAL OUTPUT (@beui/terminal-output) */}
            {activeSection === 'terminal-output' && (
              <div className="grid grid-cols-1 gap-6">
                {(
                  [
                    {
                      id: 'console',
                      title: 'BEUI Dark Console Card',
                      desc: 'Dedicated terminal chrome with `$ command` header, exit code badge, and copy.',
                      isDefault: true,
                    },
                    {
                      id: 'minimal',
                      title: 'BEUI Theme-Adaptive Console',
                      desc: 'Matches global card/border surfaces for light and dark themes.',
                      isDefault: false,
                    },
                    {
                      id: 'inline',
                      title: 'BEUI Compact Inline Stdout',
                      desc: 'Headerless compact stdout block for fast command checks.',
                      isDefault: false,
                    },
                  ] as const
                ).map((opt) => (
                  <VariantPreviewCard
                    key={opt.id}
                    title={opt.title}
                    description={opt.desc}
                    selected={state.terminalStyle === opt.id}
                    isDefault={opt.isDefault}
                    onSelect={() => applyCustomization({ terminalStyle: opt.id })}
                  >
                    <BeuiTerminalOutput
                      command="just test-guard"
                      output={
                        'running 2 tests\ntest codebase_enforces_zero_if_else_and_zero_regex ... ok\ntest release_version_surfaces_match_cargo_package_version ... ok'
                      }
                      exitCode={0}
                      variant={opt.id}
                    />
                  </VariantPreviewCard>
                ))}
              </div>
            )}

            {/* 17. TOOL RESULT (@beui/tool-result) */}
            {activeSection === 'tool-result' && (
              <div className="grid grid-cols-1 gap-6">
                {(
                  [
                    {
                      id: 'beui-card',
                      title: 'BEUI Collapsible Tool Card',
                      desc: 'Structured collapsible card with status badge, duration, and tabbed inspector.',
                      isDefault: true,
                    },
                    {
                      id: 'compact-pill',
                      title: 'BEUI Compact Pill',
                      desc: 'Collapsed inline pill chip that expands into full tabs when clicked.',
                      isDefault: false,
                    },
                    {
                      id: 'expanded-terminal',
                      title: 'BEUI Auto-Expanded Inspector',
                      desc: 'Opens tool results expanded by default so output is immediately visible.',
                      isDefault: false,
                    },
                  ] as const
                ).map((opt) => (
                  <VariantPreviewCard
                    key={opt.id}
                    title={opt.title}
                    description={opt.desc}
                    selected={state.toolResultStyle === opt.id}
                    isDefault={opt.isDefault}
                    onSelect={() => applyCustomization({ toolResultStyle: opt.id })}
                  >
                    <ToolResult
                      variant={opt.id}
                      tool={{
                        id: `demo-${opt.id}`,
                        name: 'grep_search',
                        status: 'success',
                        durationMs: 48,
                        args: { query: 'TurnState', path: 'src/agent' },
                        output: 'Found 6 matches across src/agent/agent_loop/mod.rs',
                      }}
                    />
                  </VariantPreviewCard>
                ))}
              </div>
            )}

            {/* 18. TOOL APPROVAL (@beui/tool-approval & @beui/approval-card) */}
            {activeSection === 'tool-approval' && (
              <div className="grid grid-cols-1 gap-6">
                {(
                  [
                    {
                      id: 'beui-tool-approval',
                      title: 'BEUI Tool Approval (@beui/tool-approval)',
                      desc: 'Focused inline SecurityGuard prompt with command preview and Approve / Deny actions.',
                      isDefault: true,
                    },
                    {
                      id: 'beui-approval-card',
                      title: 'BEUI Approval Audit Card (@beui/approval-card)',
                      desc: 'High-trust security card with risk badge and collapsible JSON arguments.',
                      isDefault: false,
                    },
                  ] as const
                ).map((opt) => (
                  <VariantPreviewCard
                    key={opt.id}
                    title={opt.title}
                    description={opt.desc}
                    selected={state.approvalStyle === opt.id}
                    isDefault={opt.isDefault}
                    onSelect={() => applyCustomization({ approvalStyle: opt.id })}
                  >
                    {opt.id === 'beui-approval-card' ? (
                      <div className="space-y-2">
                        <ApprovalCard
                          onChoice={() => {}}
                          prompt={{
                            id: 'sec-1',
                            toolName: 'exec_command',
                            description: 'Requires confirmation before running shell command',
                            arguments: { command: 'cargo check -p openz -j 1' },
                            status: 'pending',
                          }}
                        />
                        <ApprovalCard
                          onChoice={() => {}}
                          prompt={{
                            id: 'sec-1-resolved',
                            toolName: 'curate_skill',
                            description: 'action: "list"',
                            arguments: { action: 'list' },
                            status: 'approved',
                          }}
                        />
                      </div>
                    ) : (
                      <div className="space-y-2">
                        <ToolApproval
                          onChoice={() => {}}
                          prompt={{
                            id: 'sec-2',
                            toolName: 'exec_command',
                            description: 'Requires confirmation before running shell command',
                            arguments: { command: 'cargo check -p openz -j 1' },
                            status: 'pending',
                          }}
                        />
                        <ToolApproval
                          onChoice={() => {}}
                          prompt={{
                            id: 'sec-2-resolved',
                            toolName: 'curate_skill',
                            description: 'action: "list"',
                            arguments: { action: 'list' },
                            status: 'approved',
                          }}
                        />
                      </div>
                    )}
                  </VariantPreviewCard>
                ))}
              </div>
            )}

            {/* 19. IMAGE GENERATION (@beui/image-generation) */}
            {activeSection === 'image-generation' && (
              <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
                {(
                  [
                    {
                      id: 'beui-frame',
                      title: 'BEUI Studio Frame',
                      desc: 'Header bar with Generated Visual badge, prompt caption, fullscreen zoom & download.',
                      isDefault: true,
                    },
                    {
                      id: 'minimal',
                      title: 'BEUI Minimal Clean Image',
                      desc: 'Clean rounded visual with floating hover zoom & download actions.',
                      isDefault: false,
                    },
                  ] as const
                ).map((opt) => (
                  <VariantPreviewCard
                    key={opt.id}
                    title={opt.title}
                    description={opt.desc}
                    selected={state.imageGenStyle === opt.id}
                    isDefault={opt.isDefault}
                    onSelect={() => applyCustomization({ imageGenStyle: opt.id })}
                  >
                    <ImageGeneration
                      src={sampleSvgDataUri}
                      prompt={`OpenZ emblem (${activePalette.name} · ${activePalette.mode})`}
                      status="completed"
                      variant={opt.id}
                      className="my-0"
                    />
                  </VariantPreviewCard>
                ))}
              </div>
            )}

            {/* 20. CITATIONS (@beui/citations) */}
            {activeSection === 'citations' && (
              <div className="grid grid-cols-1 gap-6">
                {(
                  [
                    {
                      id: 'pills',
                      title: 'BEUI Source Pills + Hover Preview',
                      desc: 'Compact numbered source chips with domain hover tooltips.',
                      isDefault: true,
                    },
                    {
                      id: 'cards',
                      title: 'BEUI Expandable Source Cards',
                      desc: '2-column source card grid showing titles, domains, and external links.',
                      isDefault: false,
                    },
                    {
                      id: 'footnotes',
                      title: 'BEUI Numbered Footnote List',
                      desc: 'Clean editorial `[1]` reference list below assistant answers.',
                      isDefault: false,
                    },
                  ] as const
                ).map((opt) => (
                  <VariantPreviewCard
                    key={opt.id}
                    title={opt.title}
                    description={opt.desc}
                    selected={state.citationsStyle === opt.id}
                    isDefault={opt.isDefault}
                    onSelect={() => applyCustomization({ citationsStyle: opt.id })}
                  >
                    <BeuiCitations
                      refs={SAMPLE_CITATIONS}
                      variant={opt.id}
                      defaultOpen={opt.id === 'cards'}
                    />
                  </VariantPreviewCard>
                ))}
              </div>
            )}

            {/* 21. DATA TABLE (@beui/data-table) */}
            {activeSection === 'data-table' && (
              <div className="grid grid-cols-1 gap-6">
                {(
                  [
                    {
                      id: 'interactive',
                      title: 'BEUI Interactive Data Table',
                      desc: 'Sortable columns, row count badge, search filter, and one-click TSV copy.',
                      isDefault: true,
                    },
                    {
                      id: 'striped',
                      title: 'BEUI Striped Card Table',
                      desc: 'Alternating row tinting inside a crisp bordered card.',
                      isDefault: false,
                    },
                    {
                      id: 'minimal',
                      title: 'BEUI Minimal Editorial Table',
                      desc: 'Clean borderless horizontal-rule table for distraction-free reading.',
                      isDefault: false,
                    },
                  ] as const
                ).map((opt) => (
                  <VariantPreviewCard
                    key={opt.id}
                    title={opt.title}
                    description={opt.desc}
                    selected={state.dataTableStyle === opt.id}
                    isDefault={opt.isDefault}
                    onSelect={() => applyCustomization({ dataTableStyle: opt.id })}
                  >
                    <BeuiDataTable
                      variant={opt.id}
                      headers={['Subsystem', 'Engine', 'Latency']}
                      rows={[
                        ['Gateway', 'Axum WebSocket', '1.2ms'],
                        ['Memory', 'SQLite + FastEmbed', '4.8ms'],
                        ['UI Kit', '@beui + shadcn/ui', '0.4ms'],
                      ]}
                      className="my-0"
                    />
                  </VariantPreviewCard>
                ))}
              </div>
            )}

            {/* 22. JSON VIEWER (@beui/json-viewer) */}
            {activeSection === 'json-viewer' && (
              <div className="grid grid-cols-1 gap-6">
                {(
                  [
                    {
                      id: 'tree',
                      title: 'BEUI Collapsible JSON Tree',
                      desc: 'Interactive expandable object/array nodes with syntax-colored primitives.',
                      isDefault: true,
                    },
                    {
                      id: 'lined',
                      title: 'BEUI Formatted Pre JSON',
                      desc: 'Indented multi-line JSON block with quick copy.',
                      isDefault: false,
                    },
                    {
                      id: 'compact',
                      title: 'BEUI Single-Line Compact JSON',
                      desc: 'Minified JSON representation for dense tool telemetry.',
                      isDefault: false,
                    },
                  ] as const
                ).map((opt) => (
                  <VariantPreviewCard
                    key={opt.id}
                    title={opt.title}
                    description={opt.desc}
                    selected={state.jsonViewerStyle === opt.id}
                    isDefault={opt.isDefault}
                    onSelect={() => applyCustomization({ jsonViewerStyle: opt.id })}
                  >
                    <BeuiJsonViewer
                      label="openz.status"
                      variant={opt.id}
                      value={{
                        agent: 'openz',
                        ui: '@beui',
                        nativeTools: nativeToolCount,
                        themeMode: activePalette.mode,
                        streaming: true,
                      }}
                    />
                  </VariantPreviewCard>
                ))}
              </div>
            )}

            {/* 23. ALERT & COLLAPSIBLE (@beui/alert & @beui/collapsible) */}
            {activeSection === 'alert-collapsible' && (
              <div className="space-y-8">
                <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
                  {(
                    [
                      {
                        id: 'beui-alert',
                        title: 'BEUI Standard Callout Alert',
                        desc: 'Spacious semantic alert banner with icon and title.',
                        isDefault: true,
                      },
                      {
                        id: 'compact-pill',
                        title: 'BEUI Compact Inline Notice',
                        desc: 'Slim single-row status banner for system & tool notices.',
                        isDefault: false,
                      },
                    ] as const
                  ).map((opt) => (
                    <VariantPreviewCard
                      key={opt.id}
                      title={opt.title}
                      description={opt.desc}
                      selected={state.alertStyle === opt.id}
                      isDefault={opt.isDefault}
                      onSelect={() => applyCustomization({ alertStyle: opt.id })}
                    >
                      <BeuiAlert
                        tone="info"
                        title={opt.id === 'beui-alert' ? 'Session Restored' : undefined}
                        compact={opt.id === 'compact-pill'}
                      >
                        Connected to OpenZ Gateway at {gatewayEndpoint}.
                      </BeuiAlert>
                    </VariantPreviewCard>
                  ))}
                </div>

                <div>
                  <h2 className="mb-3 text-sm font-semibold text-foreground">
                    Collapsible Container Style (@beui/collapsible)
                  </h2>
                  <div className="grid grid-cols-1 gap-5 md:grid-cols-3">
                    {(
                      [
                        {
                          id: 'subtle',
                          title: 'Subtle Tinted Drawer',
                          desc: 'Soft muted surface for expandable disclosures.',
                          isDefault: true,
                        },
                        {
                          id: 'bordered',
                          title: 'Bordered Card Drawer',
                          desc: 'Crisp card border around header and content.',
                          isDefault: false,
                        },
                        {
                          id: 'ghost',
                          title: 'Minimal Ghost Disclosure',
                          desc: 'Borderless toggle row with clean left content.',
                          isDefault: false,
                        },
                      ] as const
                    ).map((opt) => (
                      <VariantPreviewCard
                        key={opt.id}
                        title={opt.title}
                        description={opt.desc}
                        selected={state.collapsibleStyle === opt.id}
                        isDefault={opt.isDefault}
                        onSelect={() => applyCustomization({ collapsibleStyle: opt.id })}
                      >
                        <BeuiCollapsible
                          title="Inspect runtime details"
                          variant={opt.id}
                          defaultOpen
                        >
                          <div className="text-xs text-muted-foreground">
                            {nativeToolCount} native Rust tools registered in-process.
                          </div>
                        </BeuiCollapsible>
                      </VariantPreviewCard>
                    ))}
                  </div>
                </div>
              </div>
            )}
          </div>
        </main>
      </div>
    </div>
  );
};
