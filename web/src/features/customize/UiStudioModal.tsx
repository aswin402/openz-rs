import React, { useEffect, useMemo, useState } from 'react';
import {
  Activity,
  AlertCircle,
  ArrowLeft,
  Bell,
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
  TOAST_CATEGORY_TOGGLE_OPTIONS,
  useToastStore,
} from '../../store/useToastStore';
import {
  APPROVAL_CARD_STYLE_OPTIONS,
  TOOL_CALL_STYLE_OPTIONS,
} from '../../components/assistant-ui/elements';
import { SecurityGuardPrompt } from '../../components/SecurityGuardPrompt';
import { ToolExecutionCard } from '../../components/ToolExecutionCard';
import {
  ACTIVITY_DENSITY_OPTIONS,
  ACTIVITY_MAX_HEIGHT_OPTIONS,
  ACTIVITY_ORB_OPTIONS,
  ACTIVITY_PLACEMENT_OPTIONS,
  ACTIVITY_VARIANT_OPTIONS,
  AGENT_AVATAR_OPTIONS,
  BEUI_ALERT_STYLE_OPTIONS,
  BEUI_LOADER_OPTIONS,
  AnimatedToastStack,
  AgentActivity,
  BeuiActivityOrb,
  BeuiAgentAvatar,
  BeuiAlert,
  BeuiCitations,
  BeuiCodeBlock,
  BeuiCollapsible,
  BeuiContextUsage,
  BeuiDataTable,
  BeuiFileDiff,
  BeuiJsonViewer,
  BeuiLoader,
  BeuiMessageActions,
  BeuiModelSelector,
  BeuiPlanChecklist,
  BeuiPromptInput,
  BeuiPromptSuggestions,
  BeuiSubagentCard,
  BeuiTelemetryNoticeStrip,
  BeuiTerminalOutput,
  BeuiToastStylePreview,
  BeuiUserAvatar,
  INLINE_NOTICE_STYLE_OPTIONS,
  ImageGeneration,
  MessageScroller,
  ReasoningText,
  StreamingResponse,
  TOAST_DURATION_OPTIONS,
  TOAST_MAX_VISIBLE_OPTIONS,
  TOAST_POSITION_OPTIONS,
  TOAST_VARIANT_OPTIONS,
  USER_AVATAR_OPTIONS,
  normalizeToastPosition,
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

const STUDIO_PREVIEW_ANCHOR_TS = 1_763_150_000_000;

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
    description:
      'Adaptive beui.dev stream with reactive Thought Orb, 4 variants, density, viewport clamp, and multi-surface feeds.',
    icon: Activity,
  },
  {
    id: 'animated-toast-stack',
    group: 'Agent & Reasoning',
    label: 'Toast Stack & Telemetry',
    pkg: '@beui/animated-toast-stack',
    description:
      'Layered 3D spring toast stack & inline telemetry pills for Workflow matched, Skill used, Self-improvement, Memory, and Guard approvals.',
    icon: Bell,
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
    label: 'Tool Call',
    pkg: '@assistant-ui/elements-tool-call',
    description: 'Inline collapsible tool invocations with live shimmer status and Request/Result inspection.',
    icon: Wrench,
  },
  {
    id: 'tool-approval',
    group: 'Code, Tools & Data',
    label: 'Approval Card',
    pkg: '@assistant-ui/elements-approval-card',
    description: 'SecurityGuard human-in-the-loop approval cards with command preview and live receipts.',
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
  const previewMaxTokens = useMemo(() => {
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

            {/* 10. REASONING TEXT & THINKING LOADER (@beui/reasoning-text & @beui/loader) */}
            {activeSection === 'reasoning-text' && (
              <div className="space-y-8">
                <div>
                  <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
                    <div>
                      <h2 className="text-sm font-semibold text-foreground">
                        1. Thinking Loader Animation (@beui/loader — {BEUI_LOADER_OPTIONS.length} Styles)
                      </h2>
                      <p className="text-xs text-muted-foreground">
                        Animates active Thinking, Searching, and Running Tools headers across Chat and Agent Activity.
                      </p>
                    </div>
                    <Badge variant="secondary" className="font-mono text-[10px]">
                      {state.thinkingLoader}
                    </Badge>
                  </div>

                  <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
                    {BEUI_LOADER_OPTIONS.map((loaderOpt) => {
                      const activeLoader = state.thinkingLoader === loaderOpt.id;
                      return (
                        <button
                          key={loaderOpt.id}
                          type="button"
                          onClick={() => applyCustomization({ thinkingLoader: loaderOpt.id })}
                          className={cn(
                            'flex flex-col items-start justify-between gap-3 rounded-2xl border p-3.5 text-left transition-colors',
                            activeLoader
                              ? 'border-primary/60 bg-primary/10 text-foreground ring-1 ring-primary/30'
                              : 'border-border/65 bg-card/60 text-muted-foreground hover:border-border hover:bg-card hover:text-foreground',
                          )}
                        >
                          <div className="flex h-10 w-full items-center justify-between">
                            <span className="inline-flex h-9 min-w-9 items-center justify-center rounded-xl border border-border/55 bg-background/70 px-2 text-primary">
                              <BeuiLoader
                                variant={loaderOpt.id}
                                size={
                                  loaderOpt.id === 'scramble' || loaderOpt.id === 'percent'
                                    ? 20
                                    : 18
                                }
                                labels={{ label: 'Thinking', scramble: 'THINKING' }}
                              />
                            </span>
                            {loaderOpt.isDefault && (
                              <Badge variant="outline" className="px-1.5 py-0 text-[9px]">
                                Default
                              </Badge>
                            )}
                          </div>
                          <div className="min-w-0 w-full">
                            <div className="truncate text-xs font-semibold text-foreground">
                              {loaderOpt.title}
                            </div>
                            <div className="mt-0.5 line-clamp-2 text-[10.5px] text-muted-foreground">
                              {loaderOpt.description}
                            </div>
                          </div>
                        </button>
                      );
                    })}
                  </div>
                </div>

                <div>
                  <h2 className="mb-3 text-sm font-semibold text-foreground">
                    2. Reasoning Disclosure Layout (@beui/reasoning-text)
                  </h2>
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
                          loaderVariant={state.thinkingLoader}
                          isStreaming
                          defaultOpen
                          text={
                            '1. Inspecting Cargo.toml & WebUI component tree\n2. Verifying zero-regex guard invariants\n3. Preparing concise response'
                          }
                        />
                      </VariantPreviewCard>
                    ))}
                  </div>
                </div>
              </div>
            )}

            {/* 11. AGENT ACTIVITY (@beui/agent-activity) */}
            {activeSection === 'agent-activity' && (
              <div className="space-y-8">
                {/* Live Interactive Configuration Preview */}
                <div className="rounded-2xl border border-primary/30 bg-card/65 p-4 shadow-2xs">
                  <div className="mb-2 flex flex-wrap items-center justify-between gap-2">
                    <div className="flex items-center gap-2 text-xs font-semibold text-foreground">
                      <Activity className="size-3.5 text-primary" />
                      <span>Live Preview — Active Configuration Across WebUI Surfaces</span>
                    </div>
                    <div className="flex flex-wrap items-center gap-1.5">
                      <Badge variant="secondary" className="font-mono text-[10px]">
                        {state.activityVariant}
                      </Badge>
                      <Badge variant="outline" className="font-mono text-[10px]">
                        {state.activityOrbStyle}
                      </Badge>
                      <Badge variant="outline" className="font-mono text-[10px]">
                        {state.activityDensity}
                      </Badge>
                      <Badge variant="outline" className="font-mono text-[10px]">
                        {state.activityMaxHeight}
                      </Badge>
                    </div>
                  </div>
                  <AgentActivity
                    isActive
                    mode="inline-collapsible"
                    defaultOpen
                    variant={state.activityVariant}
                    orbStyle={state.activityOrbStyle}
                    density={state.activityDensity}
                    maxHeight={state.activityMaxHeight}
                    autoCollapse={state.activityAutoCollapse}
                    showDetails={state.activityShowDetails}
                    steps={[
                      {
                        id: 'preview-think',
                        kind: 'reasoning',
                        contentType: 'reasoning',
                        title: 'Synthesizing multi-surface execution plan',
                        detail: 'Mapping @beui/agent-activity options across Chat, Drawer, Dashboard, Subagents, Inventory & Logs.',
                        payload:
                          '1. Evaluate active turn tool calls and subagent delegations\n2. Stream live elapsed durations\n3. Apply configured variant, orb indicator, and viewport clamp',
                        actor: 'openz-core',
                        status: 'info',
                        timestamp: STUDIO_PREVIEW_ANCHOR_TS - 4200,
                      },
                      {
                        id: 'preview-search',
                        kind: 'search',
                        contentType: 'search',
                        title: 'ast_grep',
                        detail: 'Scanned 260 registered tools and verified 0 regex invariants',
                        payload: 'Arguments:\n{\n  "pattern": "ToolRegistry",\n  "lang": "rust"\n}\n\nOutput:\nMatched 14 modules in 92ms',
                        meta: 'workspace',
                        status: 'success',
                        durationMs: 92,
                        durationLabel: '92ms',
                        timestamp: STUDIO_PREVIEW_ANCHOR_TS - 2800,
                      },
                      {
                        id: 'preview-subagent',
                        kind: 'tool',
                        contentType: 'tool',
                        title: 'delegate_task',
                        detail: 'Running parallel architecture verification across WebUI views',
                        actor: 'reviewer',
                        meta: 'parallel',
                        status: 'running',
                        durationMs: 640,
                        durationLabel: '640ms',
                        timestamp: STUDIO_PREVIEW_ANCHOR_TS - 900,
                      },
                    ]}
                  />
                </div>

                {/* 1. Visual Stream Variant */}
                <div>
                  <h2 className="mb-1 text-sm font-semibold text-foreground">
                    1. Activity Stream Visual Variant
                  </h2>
                  <p className="mb-4 text-xs text-muted-foreground">
                    Applies across Chat turns, the Activity Drawer, Dashboard feed, Subagents, Inventory, and Structured Logs.
                  </p>
                  <div className="grid grid-cols-1 gap-5 md:grid-cols-2">
                    {ACTIVITY_VARIANT_OPTIONS.map((opt) => (
                      <VariantPreviewCard
                        key={opt.id}
                        title={opt.title}
                        description={opt.description}
                        selected={state.activityVariant === opt.id}
                        isDefault={Boolean(opt.isDefault)}
                        onSelect={() => applyCustomization({ activityVariant: opt.id })}
                      >
                        <AgentActivity
                          isActive={opt.id === 'orb-card'}
                          mode={opt.id === 'orb-card' ? 'inline-collapsible' : 'timeline'}
                          defaultOpen
                          variant={opt.id}
                          orbStyle={state.activityOrbStyle}
                          density="compact"
                          maxHeight="compact"
                          showDetails={state.activityShowDetails}
                          steps={[
                            {
                              id: `${opt.id}-1`,
                              kind: 'search',
                              title: 'semantic_search',
                              detail: 'Indexed 18 memory nodes',
                              status: 'success',
                              durationLabel: '64ms',
                              timestamp: STUDIO_PREVIEW_ANCHOR_TS - 1800,
                            },
                            {
                              id: `${opt.id}-2`,
                              kind: 'tool',
                              title: 'cargo_check',
                              detail: 'Type-checking workspace crates',
                              actor: 'builder',
                              status: 'running',
                              durationLabel: '420ms',
                              timestamp: STUDIO_PREVIEW_ANCHOR_TS - 400,
                            },
                          ]}
                        />
                      </VariantPreviewCard>
                    ))}
                  </div>
                </div>

                {/* 2. Reactive Thought-Orb Indicator */}
                <div>
                  <h2 className="mb-1 text-sm font-semibold text-foreground">
                    2. Reactive Thought-Orb Indicator
                  </h2>
                  <p className="mb-4 text-xs text-muted-foreground">
                    Choose how active thinking, search, tool, security, and completion phases animate in headers.
                  </p>
                  <div className="grid grid-cols-1 gap-5 md:grid-cols-3">
                    {ACTIVITY_ORB_OPTIONS.map((opt) => (
                      <VariantPreviewCard
                        key={opt.id}
                        title={opt.title}
                        description={opt.description}
                        selected={state.activityOrbStyle === opt.id}
                        isDefault={Boolean(opt.isDefault)}
                        onSelect={() => applyCustomization({ activityOrbStyle: opt.id })}
                      >
                        <div className="flex items-center justify-around rounded-xl border border-border/60 bg-background/60 px-3 py-3">
                          {(
                            [
                              { phase: 'reasoning', label: 'Think', active: true },
                              { phase: 'search', label: 'Search', active: true },
                              { phase: 'tool', label: 'Tool', active: true },
                              { phase: 'complete', label: 'Done', active: false },
                            ] as const
                          ).map((sample) => (
                            <div
                              key={sample.phase}
                              className="flex flex-col items-center gap-1.5 text-[10px] text-muted-foreground"
                            >
                              <BeuiActivityOrb
                                phase={sample.phase}
                                isActive={sample.active}
                                styleVariant={opt.id}
                                size="md"
                              />
                              <span>{sample.label}</span>
                            </div>
                          ))}
                        </div>
                      </VariantPreviewCard>
                    ))}
                  </div>
                </div>

                {/* 3. Inline vs Drawer Placement */}
                <div>
                  <h2 className="mb-1 text-sm font-semibold text-foreground">
                    3. Chat Turn Placement &amp; Visibility
                  </h2>
                  <p className="mb-4 text-xs text-muted-foreground">
                    Control when @beui/agent-activity renders inline inside chat turns versus the right-hand Activity Drawer.
                  </p>
                  <div className="grid grid-cols-1 gap-5 md:grid-cols-2">
                    {ACTIVITY_PLACEMENT_OPTIONS.map((opt) => (
                      <VariantPreviewCard
                        key={opt.id}
                        title={opt.title}
                        description={opt.description}
                        selected={state.activityStyle === opt.id}
                        isDefault={Boolean(opt.isDefault)}
                        onSelect={() => applyCustomization({ activityStyle: opt.id })}
                      >
                        <div className="flex items-center justify-between rounded-lg border border-border/60 bg-background/60 px-3 py-2 text-xs">
                          <span className="font-medium text-foreground">{opt.shortLabel} Mode</span>
                          <Badge variant={opt.id === 'panel-only' ? 'outline' : 'secondary'}>
                            {opt.id === 'panel-only' ? 'Drawer Only' : 'Inline + Drawer'}
                          </Badge>
                        </div>
                      </VariantPreviewCard>
                    ))}
                  </div>
                </div>

                {/* 4. Density & Scroll Viewport Height */}
                <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
                  <div className="rounded-2xl border border-border/70 bg-card/55 p-4">
                    <h3 className="mb-1 text-xs font-semibold text-foreground">
                      Row Spacing Density
                    </h3>
                    <p className="mb-3 text-[11px] text-muted-foreground">
                      Adjust vertical rhythm and excerpt clamping inside activity lists.
                    </p>
                    <div className="grid grid-cols-2 gap-2">
                      {ACTIVITY_DENSITY_OPTIONS.map((opt) => (
                        <button
                          key={opt.id}
                          type="button"
                          onClick={() => applyCustomization({ activityDensity: opt.id })}
                          className={cn(
                            'rounded-xl border px-3 py-2.5 text-left transition-colors',
                            state.activityDensity === opt.id
                              ? 'border-primary/50 bg-primary/10 text-foreground'
                              : 'border-border/60 bg-background/50 text-muted-foreground hover:border-border hover:text-foreground',
                          )}
                        >
                          <div className="text-xs font-semibold">{opt.title}</div>
                          <div className="mt-0.5 text-[10.5px] opacity-80">{opt.description}</div>
                        </button>
                      ))}
                    </div>
                  </div>

                  <div className="rounded-2xl border border-border/70 bg-card/55 p-4">
                    <h3 className="mb-1 text-xs font-semibold text-foreground">
                      Scrollable Viewport Max-Height
                    </h3>
                    <p className="mb-3 text-[11px] text-muted-foreground">
                      Clamp long tool/log runs inside an internal scroll container.
                    </p>
                    <div className="grid grid-cols-2 gap-2">
                      {ACTIVITY_MAX_HEIGHT_OPTIONS.map((opt) => (
                        <button
                          key={opt.id}
                          type="button"
                          onClick={() => applyCustomization({ activityMaxHeight: opt.id })}
                          className={cn(
                            'rounded-xl border px-3 py-2 text-left transition-colors',
                            state.activityMaxHeight === opt.id
                              ? 'border-primary/50 bg-primary/10 text-foreground'
                              : 'border-border/60 bg-background/50 text-muted-foreground hover:border-border hover:text-foreground',
                          )}
                        >
                          <div className="flex items-center justify-between gap-1 text-xs font-semibold">
                            <span>{opt.shortLabel}</span>
                            {opt.isDefault && (
                              <span className="text-[9px] font-mono text-primary">default</span>
                            )}
                          </div>
                          <div className="mt-0.5 line-clamp-1 text-[10px] opacity-80">
                            {opt.title}
                          </div>
                        </button>
                      ))}
                    </div>
                  </div>
                </div>

                {/* 5. Behavior & Workspace Surface Toggles */}
                <div className="rounded-2xl border border-border/70 bg-card/55 p-4">
                  <h3 className="mb-3 text-xs font-semibold text-foreground">
                    Stream Behavior &amp; Multi-Surface Visibility
                  </h3>
                  <div className="space-y-3">
                    {(
                      [
                        {
                          key: 'activityAutoCollapse',
                          label: 'Auto-Collapse Inline Card on Completion',
                          desc: 'Automatically folds inline chat activity into a summary bar once a turn finishes without errors.',
                          checked: state.activityAutoCollapse,
                        },
                        {
                          key: 'activityShowDetails',
                          label: 'Show Step Excerpts, Durations & Expandable Payloads',
                          desc: 'Displays argument/output snippets, live duration badges, and click-to-expand payload inspection.',
                          checked: state.activityShowDetails,
                        },
                        {
                          key: 'showDashboardActivity',
                          label: 'Show Live Agent Activity Stream on Dashboard',
                          desc: 'Embeds a unified @beui/agent-activity feed across active sessions, orchestrations, cron jobs, and logs on the Dashboard.',
                          checked: state.showDashboardActivity,
                        },
                      ] as const
                    ).map((toggle) => (
                      <label
                        key={toggle.key}
                        className="flex cursor-pointer items-center justify-between gap-4 rounded-xl border border-border/55 bg-background/45 px-3.5 py-2.5"
                      >
                        <div>
                          <div className="text-xs font-semibold text-foreground">
                            {toggle.label}
                          </div>
                          <div className="text-[11px] text-muted-foreground">{toggle.desc}</div>
                        </div>
                        <Switch
                          checked={toggle.checked}
                          onCheckedChange={(checked) =>
                            applyCustomization({ [toggle.key]: Boolean(checked) })
                          }
                        />
                      </label>
                    ))}
                  </div>
                </div>
              </div>
            )}

            {/* 11B. ANIMATED TOAST STACK (@beui/animated-toast-stack) */}
            {activeSection === 'animated-toast-stack' && (
              <div className="space-y-8">
                {/* Master Enable & Live Trigger Playground */}
                <div className="rounded-2xl border border-primary/30 bg-card/65 p-4 shadow-2xs">
                  <div className="flex flex-wrap items-center justify-between gap-3">
                    <div className="space-y-0.5">
                      <div className="flex items-center gap-2 text-xs font-semibold text-foreground">
                        <Bell className="size-3.5 text-primary" />
                        <span>Live Telemetry Toast Stack (@beui/animated-toast-stack)</span>
                        <Badge variant="secondary" className="font-mono text-[10px]">
                          {state.toastVariant}
                        </Badge>
                        <Badge variant="outline" className="font-mono text-[10px]">
                          {normalizeToastPosition(state.toastPosition)}
                        </Badge>
                      </div>
                      <p className="text-[11px] text-muted-foreground">
                        Hover the live stack to fan out cards and pause countdowns, or swipe horizontally to dismiss.
                      </p>
                    </div>

                    <div className="flex items-center gap-2.5">
                      <span className="text-xs font-medium text-muted-foreground">
                        {state.toastEnabled ? 'Enabled' : 'Muted'}
                      </span>
                      <Switch
                        checked={state.toastEnabled}
                        onCheckedChange={(checked) =>
                          applyCustomization({ toastEnabled: Boolean(checked) })
                        }
                      />
                    </div>
                  </div>

                  <div className="mt-3.5 flex flex-wrap items-center gap-2">
                    <Button
                      type="button"
                      size="sm"
                      variant="secondary"
                      onClick={() => {
                        if (!state.toastEnabled) {
                          applyCustomization({ toastEnabled: true });
                        }
                        useToastStore.getState().pushToast({
                          dedupeKey: `demo-workflow-${Date.now()}`,
                          category: 'workflow',
                          status: 'info',
                          title: 'Workflow matched',
                          detail: 'ship-pr-until-green · verified preconditions & steps',
                          actionLabel: 'Inspect Activity',
                          onAction: () => setIsOpen(false),
                        });
                      }}
                    >
                      <span>Test Workflow Matched</span>
                    </Button>

                    <Button
                      type="button"
                      size="sm"
                      variant="secondary"
                      onClick={() => {
                        if (!state.toastEnabled) {
                          applyCustomization({ toastEnabled: true });
                        }
                        useToastStore.getState().pushToast({
                          dedupeKey: `demo-skill-${Date.now()}`,
                          category: 'skill',
                          status: 'info',
                          title: 'Skill used',
                          detail: 'openz_fullstack_playbook, rust_pro',
                          actionLabel: 'Open Skills',
                          onAction: () => {
                            useOpenZStore.getState().setActiveView('skills');
                            setIsOpen(false);
                          },
                        });
                      }}
                    >
                      <span>Test Skill Used</span>
                    </Button>

                    <Button
                      type="button"
                      size="sm"
                      variant="secondary"
                      onClick={() => {
                        if (!state.toastEnabled) {
                          applyCustomization({ toastEnabled: true });
                        }
                        useToastStore.getState().pushToast({
                          dedupeKey: `demo-curator-${Date.now()}`,
                          category: 'self_improvement',
                          status: 'success',
                          title: 'Self-improvement skill synthesized',
                          detail: 'Curator updated ~/.openz/skills/cargo_low_mem.md',
                          actionLabel: 'Open Skills',
                          onAction: () => {
                            useOpenZStore.getState().setActiveView('skills');
                            setIsOpen(false);
                          },
                        });
                      }}
                    >
                      <span>Test Self-Improvement</span>
                    </Button>

                    <Button
                      type="button"
                      size="sm"
                      onClick={() => {
                        if (!state.toastEnabled) {
                          applyCustomization({ toastEnabled: true });
                        }
                        const samples = [
                          {
                            category: 'research' as const,
                            status: 'info' as const,
                            title: 'Research brief matched',
                            detail: 'beui animated-toast-stack architecture (fresh)',
                          },
                          {
                            category: 'skill' as const,
                            status: 'info' as const,
                            title: 'Skill used',
                            detail: 'openz_orchestration_handbook',
                          },
                          {
                            category: 'workflow' as const,
                            status: 'info' as const,
                            title: 'Workflow matched',
                            detail: 'verified-webui-release',
                          },
                          {
                            category: 'self_improvement' as const,
                            status: 'success' as const,
                            title: 'Self-improvement memory updated',
                            detail: 'Saved reusable session telemetry patterns',
                          },
                        ];
                        samples.forEach((sample, idx) => {
                          globalThis.setTimeout(() => {
                            useToastStore.getState().pushToast({
                              dedupeKey: `demo-burst-${Date.now()}-${idx}`,
                              ...sample,
                            });
                          }, idx * 120);
                        });
                      }}
                    >
                      <span>Trigger 4-Card Stack Burst</span>
                    </Button>

                    <Button
                      type="button"
                      size="sm"
                      variant="ghost"
                      onClick={() => useToastStore.getState().clearActiveToasts()}
                    >
                      <span>Clear Live Stack</span>
                    </Button>
                  </div>

                  {/* Interactive Static Deck Preview (Hover to Fan Out!) */}
                  <div className="mt-4 rounded-xl border border-border/60 bg-background/60 px-4 py-3">
                    <div className="mb-1 text-center text-[10px] font-medium uppercase tracking-wider text-muted-foreground">
                      Hover deck below to preview spring fan-out geometry
                    </div>
                    <AnimatedToastStack
                      previewStatic
                      variant={state.toastVariant}
                      maxVisible={state.toastMaxVisible}
                      onDismiss={() => {}}
                      toasts={[
                        {
                          id: 'studio-toast-1',
                          category: 'workflow',
                          status: 'info',
                          title: 'Workflow matched',
                          detail: 'release-verification-pipeline (score 9.4)',
                          createdAt: 1_763_150_144_000,
                          actionLabel: 'Inspect Activity',
                          onAction: () => {},
                        },
                        {
                          id: 'studio-toast-2',
                          category: 'skill',
                          status: 'info',
                          title: 'Skill used',
                          detail: 'openz_fullstack_playbook',
                          createdAt: 1_763_150_141_000,
                          actionLabel: 'Open Skills',
                          onAction: () => {},
                        },
                        {
                          id: 'studio-toast-3',
                          category: 'self_improvement',
                          status: 'success',
                          title: 'Self-improvement memory updated',
                          detail: 'Recent conversation learnings were saved',
                          createdAt: 1_763_150_138_000,
                        },
                      ]}
                    />
                  </div>
                </div>

                {/* 1. 10 Toast Stack UI Component Styles */}
                <div>
                  <h2 className="mb-1 text-sm font-semibold text-foreground">
                    1. Toast UI Component Styles ({TOAST_VARIANT_OPTIONS.length} Components)
                  </h2>
                  <p className="mb-4 text-xs text-muted-foreground">
                    Choose from {TOAST_VARIANT_OPTIONS.length} distinct @beui/animated-toast-stack UI component styles across Light and Dark themes.
                  </p>
                  <div className="grid grid-cols-1 gap-5 md:grid-cols-2">
                    {TOAST_VARIANT_OPTIONS.map((opt) => (
                      <VariantPreviewCard
                        key={opt.id}
                        title={opt.title}
                        description={opt.description}
                        selected={state.toastVariant === opt.id}
                        isDefault={Boolean(opt.isDefault)}
                        onSelect={() => applyCustomization({ toastVariant: opt.id })}
                      >
                        <div className="space-y-2 rounded-xl border border-border/60 bg-background/55 p-3">
                          <div className="flex items-center justify-between text-[11px]">
                            <span className="font-semibold text-foreground">{opt.shortLabel} Component</span>
                            <Badge variant={state.toastVariant === opt.id ? 'default' : 'outline'}>
                              {opt.id}
                            </Badge>
                          </div>
                          <BeuiToastStylePreview variant={opt.id} />
                        </div>
                      </VariantPreviewCard>
                    ))}
                  </div>
                </div>

                {/* 2. Screen Position, Auto-Dismiss Duration & Stack Depth */}
                <div className="grid grid-cols-1 gap-6 lg:grid-cols-3">
                  <div className="rounded-2xl border border-border/70 bg-card/55 p-4">
                    <h3 className="mb-1 text-xs font-semibold text-foreground">
                      2. Viewport Anchor Placement
                    </h3>
                    <p className="mb-3 text-[11px] text-muted-foreground">
                      Place live toast decks at Top Start / Center / End or Bottom Start / Center / End.
                    </p>
                    <div className="grid grid-cols-3 gap-2">
                      {TOAST_POSITION_OPTIONS.map((opt) => {
                        const activePos = normalizeToastPosition(state.toastPosition) === opt.id;
                        return (
                          <button
                            key={opt.id}
                            type="button"
                            onClick={() => applyCustomization({ toastPosition: opt.id })}
                            className={cn(
                              'rounded-xl border px-2.5 py-2 text-left transition-colors',
                              activePos
                                ? 'border-primary/50 bg-primary/10 text-foreground'
                                : 'border-border/60 bg-background/50 text-muted-foreground hover:border-border hover:text-foreground',
                            )}
                          >
                            <div className="text-[11px] font-semibold leading-tight">{opt.shortLabel}</div>
                            <div className="mt-0.5 line-clamp-1 font-mono text-[9.5px] opacity-80">
                              {opt.isDefault ? 'default' : opt.id}
                            </div>
                          </button>
                        );
                      })}
                    </div>
                  </div>

                  <div className="rounded-2xl border border-border/70 bg-card/55 p-4">
                    <h3 className="mb-1 text-xs font-semibold text-foreground">
                      3. Auto-Dismiss Timing
                    </h3>
                    <p className="mb-3 text-[11px] text-muted-foreground">
                      Countdowns automatically pause while hovering the stack.
                    </p>
                    <div className="grid grid-cols-2 gap-2">
                      {TOAST_DURATION_OPTIONS.map((opt) => (
                        <button
                          key={opt.id}
                          type="button"
                          onClick={() => applyCustomization({ toastDuration: opt.id })}
                          className={cn(
                            'rounded-xl border px-3 py-2 text-left transition-colors',
                            state.toastDuration === opt.id
                              ? 'border-primary/50 bg-primary/10 text-foreground'
                              : 'border-border/60 bg-background/50 text-muted-foreground hover:border-border hover:text-foreground',
                          )}
                        >
                          <div className="text-xs font-semibold">{opt.shortLabel}</div>
                          <div className="mt-0.5 line-clamp-1 text-[10px] opacity-80">
                            {opt.title}
                          </div>
                        </button>
                      ))}
                    </div>
                  </div>

                  <div className="rounded-2xl border border-border/70 bg-card/55 p-4">
                    <h3 className="mb-1 text-xs font-semibold text-foreground">
                      4. Max Visible Stack Depth
                    </h3>
                    <p className="mb-3 text-[11px] text-muted-foreground">
                      Number of layered cards visible simultaneously in the stack.
                    </p>
                    <div className="grid grid-cols-3 gap-2">
                      {TOAST_MAX_VISIBLE_OPTIONS.map((opt) => (
                        <button
                          key={opt.id}
                          type="button"
                          onClick={() => applyCustomization({ toastMaxVisible: opt.id })}
                          className={cn(
                            'rounded-xl border px-3 py-2.5 text-center transition-colors',
                            state.toastMaxVisible === opt.id
                              ? 'border-primary/50 bg-primary/10 text-foreground'
                              : 'border-border/60 bg-background/50 text-muted-foreground hover:border-border hover:text-foreground',
                          )}
                        >
                          <div className="text-sm font-bold">{opt.shortLabel}</div>
                          <div className="text-[10px] opacity-80">cards</div>
                        </button>
                      ))}
                    </div>
                  </div>
                </div>

                {/* 5. Inline Chat Telemetry Notice Style */}
                <div>
                  <h2 className="mb-1 text-sm font-semibold text-foreground">
                    5. Inline Chat Telemetry Display
                  </h2>
                  <p className="mb-4 text-xs text-muted-foreground">
                    Control how Workflow matched, Skill used, and Self-improvement notices appear inside chat messages alongside live toasts.
                  </p>
                  <div className="grid grid-cols-1 gap-5 md:grid-cols-3">
                    {INLINE_NOTICE_STYLE_OPTIONS.map((opt) => (
                      <VariantPreviewCard
                        key={opt.id}
                        title={opt.title}
                        description={opt.description}
                        selected={state.inlineNoticeStyle === opt.id}
                        isDefault={Boolean(opt.isDefault)}
                        onSelect={() => applyCustomization({ inlineNoticeStyle: opt.id })}
                      >
                        {opt.id === 'toast-only' ? (
                          <div className="rounded-xl border border-dashed border-border/70 px-3 py-2.5 text-center text-[11px] text-muted-foreground">
                            Clean chat bubble — routed to Toast Stack &amp; Drawer only
                          </div>
                        ) : (
                          <BeuiTelemetryNoticeStrip
                            styleVariant={opt.id}
                            onInspectNotice={() => {}}
                            notices={[
                              {
                                id: `preview-inline-wf-${opt.id}`,
                                kind: 'workflow',
                                title: 'Workflow matched',
                                detail: 'ship-pr-until-green',
                                timestamp: STUDIO_PREVIEW_ANCHOR_TS - 2000,
                              },
                              {
                                id: `preview-inline-sk-${opt.id}`,
                                kind: 'skill',
                                title: 'Skill used',
                                detail: 'rust_pro',
                                timestamp: STUDIO_PREVIEW_ANCHOR_TS - 1000,
                              },
                            ]}
                          />
                        )}
                      </VariantPreviewCard>
                    ))}
                  </div>
                </div>

                {/* 6. Granular Event Category Routing Switches */}
                <div className="rounded-2xl border border-border/70 bg-card/55 p-4">
                  <h3 className="mb-1 text-xs font-semibold text-foreground">
                    6. Per-Category Toast Triggers
                  </h3>
                  <p className="mb-3 text-[11px] text-muted-foreground">
                    Choose exactly which agent runtime events trigger popup toasts. Muted categories are still logged quietly in the Bell Notification Center.
                  </p>
                  <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
                    {TOAST_CATEGORY_TOGGLE_OPTIONS.map((toggle) => (
                      <label
                        key={toggle.key}
                        className="flex cursor-pointer items-center justify-between gap-4 rounded-xl border border-border/55 bg-background/45 px-3.5 py-2.5"
                      >
                        <div>
                          <div className="text-xs font-semibold text-foreground">
                            {toggle.title}
                          </div>
                          <div className="text-[11px] text-muted-foreground">
                            {toggle.description}
                          </div>
                        </div>
                        <Switch
                          checked={Boolean(state[toggle.key])}
                          onCheckedChange={(checked) =>
                            applyCustomization({ [toggle.key]: Boolean(checked) })
                          }
                        />
                      </label>
                    ))}
                  </div>
                </div>
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

            {/* 17. TOOL CALL (@beui/agent-activity & @assistant-ui/elements-tool-call) */}
            {activeSection === 'tool-result' && (
              <div className="grid grid-cols-1 gap-6">
                {TOOL_CALL_STYLE_OPTIONS.map((opt) => (
                  <VariantPreviewCard
                    key={opt.id}
                    title={opt.title}
                    description={opt.description}
                    selected={state.toolResultStyle === opt.id}
                    isDefault={opt.isDefault}
                    onSelect={() => applyCustomization({ toolResultStyle: opt.id })}
                  >
                    {opt.id === 'beui-activity' ? (
                      <AgentActivity
                        mode="inline-collapsible"
                        defaultOpen
                        autoCollapse={false}
                        variant="orb-card"
                        orbStyle={state.activityOrbStyle}
                        loaderVariant={state.thinkingLoader}
                        steps={[
                          {
                            id: 'preview-read-inv',
                            kind: 'tool',
                            contentType: 'tool',
                            title: 'openz_inventory',
                            toolAction: 'read',
                            toolTarget: 'openz_inventory',
                            status: 'success',
                            timestamp: STUDIO_PREVIEW_ANCHOR_TS - 900,
                          },
                          {
                            id: 'preview-write-skill',
                            kind: 'tool',
                            contentType: 'tool',
                            title: 'curate_skill',
                            toolAction: 'write',
                            toolTarget: 'youtube_song_playback',
                            status: 'success',
                            timestamp: STUDIO_PREVIEW_ANCHOR_TS - 400,
                          },
                        ]}
                      />
                    ) : (
                      <div className="flex flex-col gap-2">
                        <ToolExecutionCard
                          key={`demo-ok-${opt.id}`}
                          styleOverride={opt.id}
                          tool={{
                            id: `demo-ok-${opt.id}`,
                            name: 'grep_search',
                            status: 'success',
                            durationMs: 48,
                            args: { query: 'TurnState', path: 'src/agent' },
                            output: 'Found 6 matches across src/agent/agent_loop/mod.rs',
                          }}
                        />
                        <ToolExecutionCard
                          key={`demo-run-${opt.id}`}
                          styleOverride={opt.id}
                          tool={{
                            id: `demo-run-${opt.id}`,
                            name: 'exec_command',
                            status: 'running',
                            args: { command: 'cargo check -p openz -j 1' },
                          }}
                        />
                        <ToolExecutionCard
                          key={`demo-err-${opt.id}`}
                          styleOverride={opt.id}
                          tool={{
                            id: `demo-err-${opt.id}`,
                            name: 'cargo_manager',
                            status: 'error',
                            durationMs: 312,
                            args: { action: 'test', package: 'openz' },
                            error: 'Test assertion failed in session::tests::compaction_roundtrip',
                          }}
                        />
                      </div>
                    )}
                  </VariantPreviewCard>
                ))}
              </div>
            )}

            {/* 18. APPROVAL CARD (@assistant-ui/elements-approval-card) */}
            {activeSection === 'tool-approval' && (
              <div className="grid grid-cols-1 gap-6">
                {APPROVAL_CARD_STYLE_OPTIONS.map((opt) => (
                  <VariantPreviewCard
                    key={opt.id}
                    title={opt.title}
                    description={opt.description}
                    selected={state.approvalStyle === opt.id}
                    isDefault={opt.isDefault}
                    onSelect={() => applyCustomization({ approvalStyle: opt.id })}
                  >
                    <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
                      <SecurityGuardPrompt
                        styleOverride={opt.id}
                        onChoice={() => {}}
                        prompt={{
                          id: `sec-pending-${opt.id}`,
                          toolName: 'exec_command',
                          description: 'Requires confirmation before running shell command',
                          arguments: { command: 'cargo check -p openz -j 1' },
                          status: 'pending',
                        }}
                      />
                      <div className="flex flex-col gap-3">
                        <SecurityGuardPrompt
                          styleOverride={opt.id}
                          stateOverride="running"
                          onChoice={() => {}}
                          prompt={{
                            id: `sec-running-${opt.id}`,
                            toolName: 'exec_command',
                            description: '',
                            arguments: { command: 'just test openz' },
                            status: 'approved',
                          }}
                        />
                        <SecurityGuardPrompt
                          styleOverride={opt.id}
                          stateOverride="done"
                          onChoice={() => {}}
                          prompt={{
                            id: `sec-done-${opt.id}`,
                            toolName: 'curate_skill',
                            description: '',
                            arguments: { action: 'save', name: 'opencaveman' },
                            status: 'approved',
                          }}
                        />
                      </div>
                    </div>
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
                <div>
                  <h2 className="mb-1 text-sm font-semibold text-foreground">
                    1. Animated Alert Surface (@beui/alert — {BEUI_ALERT_STYLE_OPTIONS.length} Styles)
                  </h2>
                  <p className="mb-4 text-xs text-muted-foreground">
                    Powered by official @beui/alert with spring layout transitions, icon morphing, and animated dismiss.
                  </p>
                  <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
                    {BEUI_ALERT_STYLE_OPTIONS.map((opt) => (
                      <VariantPreviewCard
                        key={opt.id}
                        title={opt.title}
                        description={opt.description}
                        selected={state.alertStyle === opt.id}
                        isDefault={Boolean(opt.isDefault)}
                        onSelect={() => applyCustomization({ alertStyle: opt.id })}
                      >
                        <div className="space-y-2.5">
                          <BeuiAlert
                            tone="info"
                            styleVariant={opt.id}
                            title="Session Restored"
                          >
                            Connected to OpenZ Gateway at {gatewayEndpoint}.
                          </BeuiAlert>
                          {opt.id !== 'compact-pill' && (
                            <BeuiAlert
                              tone="success"
                              styleVariant={opt.id}
                              title="Self-improvement consolidated"
                            >
                              Updated skill bundle on disk with zero conflicts.
                            </BeuiAlert>
                          )}
                        </div>
                      </VariantPreviewCard>
                    ))}
                  </div>
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
