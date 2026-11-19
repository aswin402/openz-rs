import React, { useEffect, useMemo, useState } from 'react';
import {
  Activity,
  AlertCircle,
  AlertTriangle,
  ArrowLeft,
  ArrowRightLeft,
  Bell,
  Bot,
  BookOpen,
  Braces,
  Brain,
  Check,
  ChevronsUpDown,
  CircleDot,
  Clock,
  Code2,
  Columns2,
  Cpu,
  Database,
  Download,
  FileDiff as FileDiffIcon,
  FilePlus2,
  FileSpreadsheet,
  FolderTree,
  Gauge,
  Globe,
  Image as ImageIcon,
  Inbox,
  Lightbulb,
  Link2,
  ListChecks,
  Loader,
  MapPin,
  MessageSquare,
  Monitor,
  Moon,
  MousePointerClick,
  Palette,
  PanelRight,
  RotateCcw,
  Scale,
  ScrollText,
  Search,
  Send,
  SendHorizontal,
  Settings,
  ShieldCheck,
  Sigma,
  Sparkles,
  Sun,
  Table2,
  Tag,
  Terminal,
  Trash2,
  Upload,
  User,
  UserCheck,
  Wrench,
  X,
} from 'lucide-react';
import { useThemeStore, type Theme } from '../../store/useThemeStore';
import { useResolvedTheme } from '../../shared/lib/theme';
import {
  THEME_TRANSITION_STYLE_OPTIONS,
  runThemeViewTransition,
  type ThemeTransitionStyleChoice,
} from '../../shared/lib/themeTransition';
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
import {
  ClassicThemeToggleIcon,
  SpectrumNavListCard,
} from '../../components/registries/modernRegistries';
import { Badge } from '../../components/ui/badge';
import { Button } from '../../components/ui/button';
import { Input } from '../../components/ui/input';
import { Switch } from '../../components/ui/switch';
import { cn } from '../../lib/utils';
import { ExtendedElementsGalleries } from './ExtendedElementsGalleries';
import { VariantPreviewCard } from './VariantPreviewCard';

export type StudioMenuGroup =
  | 'Appearance'
  | 'Composer & Chat'
  | 'Agent & Reasoning'
  | 'Notifications & Alerts'
  | 'Code, Tools & Data';

export interface StudioMenuEntry {
  id: CustomizeUiSectionId;
  group: StudioMenuGroup;
  label: string;
  pkg: string;
  description: string;
  icon: React.ComponentType<{ className?: string }>;
}

const STUDIO_PREVIEW_ANCHOR_TS = 1_763_150_000_000;

export const STUDIO_MENU_ITEMS: StudioMenuEntry[] = [
  {
    id: 'theme',
    group: 'Appearance',
    label: 'Global Theme',
    pkg: '@beui/theme',
    description: 'Select from 19 curated dual-mode color themes (both Light & Dark palettes) across OpenZ.',
    icon: Palette,
  },
  {
    id: 'badge-style',
    group: 'Appearance',
    label: 'Badge Style',
    pkg: '@uiarc/badge',
    description: '6 global badge styles across Minimal Hairline, Soft Capsule, Monospace Tag, Tinted, UIArc Badge, and Spectrum Status.',
    icon: Tag,
  },
  {
    id: 'dock-toolbar',
    group: 'Appearance',
    label: 'Dock Toolbar',
    pkg: '@kokonutui/toolbar',
    description: 'Floating right dock toolbar with spring pill selection glide, Rare-UI notification bell, and Classic theme toggle.',
    icon: PanelRight,
  },
  {
    id: 'drawer-motion',
    group: 'Appearance',
    label: 'Drawer Motion',
    pkg: '@kokonutui/smooth-drawer',
    description: 'Spring-damped drawer physics across Agent Activity, Live Logs, and Notification Center drawers.',
    icon: Columns2,
  },
  {
    id: 'nav-menu',
    group: 'Appearance',
    label: 'Navigation Menu',
    pkg: '@spectrumui/nav-list-card',
    description: 'Animated vertical navigation menu style across Customize UI and Gateway Settings sidebars.',
    icon: ListChecks,
  },
  {
    id: 'avatars',
    group: 'Appearance',
    label: 'Agent Avatars',
    pkg: '@beui/agent-avatar',
    description: 'Choose from 13 OpenZ Fox & BEUI agent emblems displayed next to assistant responses.',
    icon: User,
  },
  {
    id: 'user-avatars',
    group: 'Appearance',
    label: 'User Avatars',
    pkg: '@beui/user-avatar',
    description: 'Choose from 6 outer-right user avatar emblems displayed next to your prompts.',
    icon: UserCheck,
  },
  {
    id: 'chat-width',
    group: 'Appearance',
    label: 'Chat Track Width',
    pkg: '@beui/chat-layout',
    description: 'Conversation column width across Balanced (820px), Wide Studio (1080px), and Full Edge-to-Edge.',
    icon: Columns2,
  },
  {
    id: 'delete-button',
    group: 'Appearance',
    label: 'Delete Button',
    pkg: 'rare-ui/delete-button',
    description: 'Destructive action confirmation style across Sessions, Notifications, and workspace items.',
    icon: Trash2,
  },
  {
    id: 'prompt-input',
    group: 'Composer & Chat',
    label: 'Prompt Input',
    pkg: '@tweenly/ai-input',
    description: 'Floating composer surface across @beui, Tweenly Fluid AI Input, Rare-UI Prompt Box, and KokonutUI AI Prompt.',
    icon: Send,
  },
  {
    id: 'token-alternatives',
    group: 'Composer & Chat',
    label: 'Token Alternatives',
    pkg: '@kokonutui/ai-token-alternatives',
    description: 'Interactive model and caveman token budget comparison chips inside the floating composer.',
    icon: Cpu,
  },
  {
    id: 'send-button',
    group: 'Composer & Chat',
    label: 'Send Button',
    pkg: '@beui/send-button',
    description: 'Icon-only composer submit button shape and glyph style.',
    icon: SendHorizontal,
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
    id: 'recommendation-card',
    group: 'Composer & Chat',
    label: 'Recommendation Card',
    pkg: 'assistant-ui/recommendation-card',
    description: 'Actionable next-step suggestion cards and follow-up action chips.',
    icon: Lightbulb,
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
    description: 'Auto-stick scroll behavior while streaming tokens and reading long sessions.',
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
    description: 'Live token-streaming cursor across @beui, Rare-UI, BeautifulUI, AICSS, and assistant-ui.',
    icon: Sparkles,
  },
  {
    id: 'loading-state',
    group: 'Agent & Reasoning',
    label: 'Loading State',
    pkg: 'assistant-ui/loading-state',
    description: 'Pre-first-token assistant loading indicator across assistant-ui, KokonutUI AI Loading, and Tweenly AI Thinking.',
    icon: Loader,
  },
  {
    id: 'thinking-loader',
    group: 'Agent & Reasoning',
    label: 'Thinking Loader',
    pkg: '@beui/loader',
    description: '23 animated loader indicators across @beui/loader, Tweenly Wave, BeautifulUI, KokonutUI, AICSS, and thinking-orbs.',
    icon: Loader,
  },
  {
    id: 'reasoning-text',
    group: 'Agent & Reasoning',
    label: 'Reasoning Text',
    pkg: '@tweenly/ai-thinking',
    description: 'Chain-of-thought disclosure across @beui, Tweenly AI Thinking, KokonutUI AI Loading, AICSS, and BeautifulUI.',
    icon: Brain,
  },
  {
    id: 'agent-activity',
    group: 'Agent & Reasoning',
    label: 'Agent Activity',
    pkg: '@beui/agent-activity',
    description: 'Multi-step activity stream across @beui, assistant-ui Run Activity, and Tool Timeline.',
    icon: Activity,
  },
  {
    id: 'activity-orb',
    group: 'Agent & Reasoning',
    label: 'Activity Orb',
    pkg: 'aicss/orbs',
    description: '10 reactive phase orbs across @beui, thinking-orbs 2D Canvas, AICSS Orbs, and Rare-UI Status.',
    icon: CircleDot,
  },
  {
    id: 'activity-placement',
    group: 'Agent & Reasoning',
    label: 'Activity Placement',
    pkg: '@beui/activity-placement',
    description: 'Control where Agent Activity streams appear across inline chat turns, the right Drawer, and Dashboard.',
    icon: PanelRight,
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
    id: 'agent-handoff',
    group: 'Agent & Reasoning',
    label: 'Agent Handoff',
    pkg: 'assistant-ui/agent-handoff',
    description: 'Specialist delegation transfer cards when OpenZ forks or delegates tasks.',
    icon: ArrowRightLeft,
  },
  {
    id: 'timeline',
    group: 'Agent & Reasoning',
    label: 'Event Timeline',
    pkg: 'assistant-ui/timeline',
    description: 'Chronological milestone timeline for completed, active, and upcoming workflow stages.',
    icon: Clock,
  },
  {
    id: 'animated-toast-stack',
    group: 'Notifications & Alerts',
    label: 'Toast Stack',
    pkg: 'rare-ui/toast',
    description: '16 layered spring notification styles across @beui, Rare-UI Toast, UIArc Toast Stack, and Spectrum Toast Stack.',
    icon: Bell,
  },
  {
    id: 'notification-center',
    group: 'Notifications & Alerts',
    label: 'Notification Center',
    pkg: '@uiarc/notification-center',
    description: 'Notification history center across UIArc Notification Center, Tweenly Notification Feed, and BEUI Minimal.',
    icon: Inbox,
  },
  {
    id: 'toast-placement',
    group: 'Notifications & Alerts',
    label: 'Toast Placement',
    pkg: '@beui/toast-placement',
    description: 'Screen corner/edge anchor placement, auto-dismiss timing, and maximum visible stack depth.',
    icon: MapPin,
  },
  {
    id: 'telemetry-badges',
    group: 'Notifications & Alerts',
    label: 'Telemetry Badges',
    pkg: 'assistant-ui/memory-chips',
    description: 'Minimal whisper rows, Memory Chips, BeautifulUI Tool Chips, and Rare-UI AI Status badges.',
    icon: Tag,
  },
  {
    id: 'background-inbox',
    group: 'Notifications & Alerts',
    label: 'Background Inbox',
    pkg: 'assistant-ui/background-inbox',
    description: 'Async task tray tracking background Curator, Cron, and multi-session runs.',
    icon: Inbox,
  },
  {
    id: 'alert-collapsible',
    group: 'Notifications & Alerts',
    label: 'Animated Alert',
    pkg: '@beui/alert',
    description: 'Spring-animated status callouts with icon transitions and smooth dismiss.',
    icon: AlertCircle,
  },
  {
    id: 'error-state',
    group: 'Notifications & Alerts',
    label: 'Error State',
    pkg: 'assistant-ui/error-state',
    description: 'Quiet failure banners with integrated one-click Retry and diagnostic traces.',
    icon: AlertTriangle,
  },
  {
    id: 'collapsible',
    group: 'Notifications & Alerts',
    label: 'Collapsible',
    pkg: '@beui/collapsible',
    description: 'Expandable disclosure container styles for runtime metadata and nested details.',
    icon: ChevronsUpDown,
  },
  {
    id: 'code-block',
    group: 'Code, Tools & Data',
    label: 'Code Block',
    pkg: '@beui/code-block',
    description: '7 syntax-highlighted code styles across @beui, Rare-UI, BeautifulUI, JolyUI, and AICSS.',
    icon: Code2,
  },
  {
    id: 'file-diff',
    group: 'Code, Tools & Data',
    label: 'File Diff',
    pkg: 'assistant-ui/code-diff',
    description: 'Unified, split side-by-side, compact, and assistant-ui Code Diff viewers.',
    icon: FileDiffIcon,
  },
  {
    id: 'file-tree',
    group: 'Code, Tools & Data',
    label: 'File Tree',
    pkg: 'assistant-ui/file-tree',
    description: 'Hierarchical folder tree summarizing created, modified, and inspected workspace files.',
    icon: FolderTree,
  },
  {
    id: 'artifact-card',
    group: 'Code, Tools & Data',
    label: 'Artifact Card',
    pkg: 'assistant-ui/artifact-card',
    description: 'Interactive cards for generated files, reports, and downloadable artifacts.',
    icon: FilePlus2,
  },
  {
    id: 'terminal-output',
    group: 'Code, Tools & Data',
    label: 'Terminal Output',
    pkg: 'assistant-ui/terminal-block',
    description: 'Streaming shell console cards across @beui and assistant-ui Terminal Block.',
    icon: Terminal,
  },
  {
    id: 'tool-result',
    group: 'Code, Tools & Data',
    label: 'Tool Call',
    pkg: 'beautifului/tool-chips',
    description: '6 tool displays across @beui, BeautifulUI Tool Chips, KokonutUI, and assistant-ui.',
    icon: Wrench,
  },
  {
    id: 'tool-approval',
    group: 'Code, Tools & Data',
    label: 'Approval Card',
    pkg: '@kokonutui/ai-tool-approval',
    description: 'SecurityGuard human-in-the-loop cards across assistant-ui and KokonutUI.',
    icon: ShieldCheck,
  },
  {
    id: 'web-search',
    group: 'Code, Tools & Data',
    label: 'Web Search',
    pkg: 'assistant-ui/web-search',
    description: 'Live web search query & domain result cards across assistant-ui and Rare-UI.',
    icon: Globe,
  },
  {
    id: 'retrieval-chunks',
    group: 'Code, Tools & Data',
    label: 'Retrieval Chunks',
    pkg: 'assistant-ui/retrieval-chunks',
    description: 'Collapsible RAG & semantic search context passages with match scores.',
    icon: Database,
  },
  {
    id: 'research-report',
    group: 'Code, Tools & Data',
    label: 'Research Report',
    pkg: 'assistant-ui/research-report',
    description: 'Sectioned deep-research outlines with per-section source attribution.',
    icon: BookOpen,
  },
  {
    id: 'inline-citations',
    group: 'Code, Tools & Data',
    label: 'Inline Citations',
    pkg: 'aicss/inline-citations',
    description: 'Hoverable inline prose citation pills across AICSS, KokonutUI, and Minimal Superscript.',
    icon: Link2,
  },
  {
    id: 'citations',
    group: 'Code, Tools & Data',
    label: 'Citations',
    pkg: '@kokonutui/ai-citations',
    description: 'Footer source decks across @beui, KokonutUI Citations, and Minimal Inline.',
    icon: BookOpen,
  },
  {
    id: 'image-generation',
    group: 'Code, Tools & Data',
    label: 'Image Generation',
    pkg: 'assistant-ui/image-generation',
    description: 'Rendered PNG/SVG media frames across @beui and assistant-ui Image Generation.',
    icon: ImageIcon,
  },
  {
    id: 'math-block',
    group: 'Code, Tools & Data',
    label: 'Math Block',
    pkg: 'assistant-ui/math-block',
    description: 'Display KaTeX equation framing with one-click LaTeX copy.',
    icon: Sigma,
  },
  {
    id: 'spec-sheet',
    group: 'Code, Tools & Data',
    label: 'Spec Sheet',
    pkg: 'assistant-ui/spec-sheet',
    description: 'Structured 2-column specification displays for runtime and model properties.',
    icon: FileSpreadsheet,
  },
  {
    id: 'comparison-card',
    group: 'Code, Tools & Data',
    label: 'Comparison Table',
    pkg: 'aicss/comparison-table',
    description: 'Side-by-side matrices & option cards across AICSS Comparison Table and assistant-ui.',
    icon: Scale,
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
    id: 'memory-chips',
    group: 'Code, Tools & Data',
    label: 'Memory Chips',
    pkg: 'assistant-ui/memory-chips',
    description: 'Inline memory fact, graph entity, and skill recall chips rendered above assistant turns.',
    icon: Database,
  },
  {
    id: 'json-viewer',
    group: 'Code, Tools & Data',
    label: 'JSON Viewer',
    pkg: '@uiarc/json-viewer',
    description: 'Interactive collapsible JSON tree and searchable UIArc JSON Inspector for structured tool payloads.',
    icon: Braces,
  },
];

const MENU_ITEMS = STUDIO_MENU_ITEMS;

export const STUDIO_MENU_GROUPS: StudioMenuGroup[] = [
  'Appearance',
  'Composer & Chat',
  'Agent & Reasoning',
  'Notifications & Alerts',
  'Code, Tools & Data',
];

const MENU_GROUPS = STUDIO_MENU_GROUPS;

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

  const handleSelectMode = (mode: Theme, overrideStyle?: ThemeTransitionStyleChoice) => {
    runThemeViewTransition(
      () => {
        setThemeMode(mode);
      },
      { style: overrideStyle ?? state.themeTransitionStyle },
    );
  };

  const handleChooseGlobalTheme = (themeId: string, explicitMode?: 'dark' | 'light') => {
    runThemeViewTransition(
      () => {
        const family = selectThemePreset(themeId);
        if (!family) return;
        if (explicitMode) {
          setThemeMode(explicitMode);
        }
      },
      { style: state.themeTransitionStyle },
    );
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
            size="icon-sm"
            onClick={() => handleSelectMode(resolvedMode === 'dark' ? 'light' : 'dark')}
            title={resolvedMode === 'dark' ? 'Switch to Light mode' : 'Switch to Dark mode'}
            aria-label={resolvedMode === 'dark' ? 'Switch to Light mode' : 'Switch to Dark mode'}
          >
            <ClassicThemeToggleIcon isDark={resolvedMode === 'dark'} />
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
                <div key={groupName} className="flex shrink-0 gap-1 md:mb-4 md:flex-col md:gap-1">
                  <div className="hidden px-2.5 pb-1 text-[10.5px] font-semibold uppercase tracking-wider text-muted-foreground/75 md:block">
                    {groupName}
                  </div>
                  {/* Mobile horizontal pills */}
                  <div className="flex gap-1 md:hidden">
                    {items.map((item) => {
                      const Icon = item.icon;
                      const active = item.id === activeSection;
                      return (
                        <button
                          key={item.id}
                          type="button"
                          onClick={() => setActiveSection(item.id)}
                          className={cn(
                            'flex shrink-0 items-center gap-2 rounded-md px-2.5 py-1.5 text-left text-xs transition-colors',
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
                  {/* Desktop @spectrumui/nav-list-card */}
                  <div className="hidden md:block">
                    <SpectrumNavListCard
                      items={items.map((item) => ({
                        id: item.id,
                        label: item.label,
                        icon: item.icon,
                      }))}
                      activeId={activeSection}
                      onSelect={(id) => setActiveSection(id as CustomizeUiSectionId)}
                      variant={state.navMenuStyle}
                      layoutGroupId={`studio-nav-${groupName}`}
                    />
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

                <div className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-border/80 bg-card p-4">
                  <div>
                    <div className="text-xs font-semibold text-foreground">
                      Theme Transition Animation
                    </div>
                    <p className="text-xs text-muted-foreground">
                      Choose directional side curtain, center horizon bloom, or radial click-origin reveal
                    </p>
                  </div>
                  <div className="flex flex-wrap items-center gap-1.5">
                    <div className="inline-flex flex-wrap rounded-lg border border-border bg-muted/40 p-0.5 text-xs">
                      {THEME_TRANSITION_STYLE_OPTIONS.map((opt) => {
                        const active = state.themeTransitionStyle === opt.id;
                        return (
                          <button
                            key={opt.id}
                            type="button"
                            title={opt.description}
                            onClick={() => applyCustomization({ themeTransitionStyle: opt.id })}
                            className={cn(
                              'rounded-md px-2.5 py-1.5 font-medium transition-colors',
                              active
                                ? 'bg-card text-foreground shadow-2xs'
                                : 'text-muted-foreground hover:text-foreground',
                            )}
                          >
                            {opt.shortLabel}
                          </button>
                        );
                      })}
                    </div>
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      onClick={() =>
                        handleSelectMode(
                          resolvedMode === 'dark' ? 'light' : 'dark',
                          state.themeTransitionStyle,
                        )
                      }
                      title="Test the selected theme transition animation"
                    >
                      <Sparkles className="size-3.5 text-primary" />
                      <span>Preview</span>
                    </Button>
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
                    {
                      id: 'rare-prompt-box',
                      title: 'Rare-UI Floating Prompt Box',
                      desc: 'swamimalode07/rare-ui/prompt-box — Frosted glass rounded composer with soft focus ring.',
                      isDefault: false,
                    },
                    {
                      id: 'kokonut-ai-prompt',
                      title: 'KokonutUI Studio Prompt',
                      desc: '@kokonutui/ai-prompt — Low-noise hairline studio bar that elevates on focus.',
                      isDefault: false,
                    },
                    {
                      id: 'tweenly-ai-input',
                      title: 'Tweenly Fluid AI Input',
                      desc: '@tweenly/ai-input — Frosted floating composer with spring focus ring and glow halo.',
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
            )}

            {/* 2B. SEND BUTTON (@beui/send-button) */}
            {activeSection === 'send-button' && (
              <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
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
                        variant={state.promptInputStyle}
                        sendButtonVariant={opt.id}
                      />
                    </div>
                  </VariantPreviewCard>
                ))}
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
            )}

            {/* 6B. CHAT TRACK WIDTH (@beui/chat-layout) */}
            {activeSection === 'chat-width' && (
              <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
                {(
                  [
                    {
                      id: 'standard',
                      title: 'Balanced (820px)',
                      desc: 'Optimal 820px reading column with outer Fox & User gutters.',
                      widthClass: 'w-3/5',
                      specLabel: '820px column',
                      isDefault: true,
                    },
                    {
                      id: 'wide',
                      title: 'Wide Studio (1080px)',
                      desc: 'Expanded 1080px track ideal for split diffs & wide tables.',
                      widthClass: 'w-4/5',
                      specLabel: '1080px column',
                      isDefault: false,
                    },
                    {
                      id: 'full',
                      title: 'Full Edge-to-Edge',
                      desc: 'Spans full available viewport width across ultra-wide monitors.',
                      widthClass: 'w-full',
                      specLabel: '100% viewport',
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
                    <div className="flex w-full items-center justify-center rounded-xl border border-border/65 bg-card/55 p-3">
                      <div
                        className={cn(
                          'space-y-2 rounded-lg border border-primary/40 bg-primary/8 p-2.5 transition-all',
                          opt.widthClass,
                        )}
                      >
                        <div className="flex items-center justify-between text-[10px] font-mono text-primary">
                          <Columns2 className="size-3.5" />
                          <span>{opt.specLabel}</span>
                        </div>
                        <div className="h-2 w-4/5 rounded-full bg-foreground/20" />
                        <div className="ml-auto h-2 w-1/2 rounded-full bg-primary/45" />
                      </div>
                    </div>
                  </VariantPreviewCard>
                ))}
              </div>
            )}

            {/* AGENT AVATARS (@beui/agent-avatar) */}
            {activeSection === 'avatars' && (
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
            )}

            {/* USER AVATARS (@beui/user-avatar) */}
            {activeSection === 'user-avatars' && (
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
                      id: 'rare-ink',
                      title: 'Rare-UI Luminous Ink Caret',
                      desc: 'swamimalode07/rare-ui/streaming-response — Soft glowing ink caret along the live token edge.',
                      isDefault: false,
                    },
                    {
                      id: 'beautiful-stream',
                      title: 'BeautifulUI Streaming Cadence',
                      desc: 'beautifului.dev/r/streaming-text — Concentric live pulse dot for streaming output.',
                      isDefault: false,
                    },
                    {
                      id: 'aicss-stream',
                      title: 'AICSS Streaming Text',
                      desc: 'aicss.dev/r/streaming-text — Smooth live streaming indicator with zero layout shift.',
                      isDefault: false,
                    },
                    {
                      id: 'assistant-stream',
                      title: 'Assistant-UI Streaming Text',
                      desc: 'assistant-ui/elements/streaming-text — Minimal monochrome hairline caret.',
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

            {/* 10A. THINKING LOADER (@beui/loader) */}
            {activeSection === 'thinking-loader' && (
              <div className="grid grid-cols-1 gap-5 sm:grid-cols-2 lg:grid-cols-3">
                {BEUI_LOADER_OPTIONS.map((loaderOpt) => (
                  <VariantPreviewCard
                    key={loaderOpt.id}
                    title={loaderOpt.title}
                    description={loaderOpt.description}
                    selected={state.thinkingLoader === loaderOpt.id}
                    isDefault={Boolean(loaderOpt.isDefault)}
                    onSelect={() => applyCustomization({ thinkingLoader: loaderOpt.id })}
                  >
                    <div className="flex flex-col items-center gap-3 py-1">
                      <span className="inline-flex h-12 min-w-12 items-center justify-center rounded-2xl border border-border/65 bg-card px-3.5 text-primary shadow-2xs">
                        <BeuiLoader
                          variant={loaderOpt.id}
                          size={
                            loaderOpt.id === 'scramble' || loaderOpt.id === 'percent'
                              ? 22
                              : 20
                          }
                          labels={{ label: 'Thinking', scramble: 'THINKING' }}
                        />
                      </span>
                      <div className="flex w-full items-center justify-between gap-2 rounded-xl border border-border/65 bg-card/75 px-3 py-2 text-xs">
                        <div className="flex min-w-0 items-center gap-2 text-foreground/90">
                          <span className="inline-flex shrink-0 items-center text-primary">
                            <BeuiLoader
                              variant={loaderOpt.id}
                              size={14}
                              labels={{ label: 'Thinking', scramble: 'THINK' }}
                            />
                          </span>
                          <span className="truncate font-medium">Working through it…</span>
                        </div>
                        <span className="shrink-0 font-mono text-[10px] text-muted-foreground">
                          live
                        </span>
                      </div>
                    </div>
                  </VariantPreviewCard>
                ))}
              </div>
            )}

            {/* 10B. REASONING TEXT (@beui/reasoning-text) */}
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
                    {
                      id: 'aicss-reasoning',
                      title: 'AICSS Thinking & Reasoning Frame',
                      desc: 'aicss.dev/r/thinking-reasoning — Bordered inner-monologue disclosure with step counter.',
                      isDefault: false,
                    },
                    {
                      id: 'beautiful-thinking',
                      title: 'BeautifulUI Thinking State',
                      desc: 'beautifului.dev/r/thinking-state — Ghost borderless reasoning trace for clean prose.',
                      isDefault: false,
                    },
                    {
                      id: 'minimal-whisper',
                      title: 'Minimal Whisper Line',
                      desc: 'Unboxed single-line thought summary with zero border.',
                      isDefault: false,
                    },
                    {
                      id: 'tweenly-thinking',
                      title: 'Tweenly AI Thinking Wave',
                      desc: '@tweenly/ai-thinking — Harmonic equalizer wave pill with expandable thought stream.',
                      isDefault: false,
                    },
                    {
                      id: 'kokonut-loading',
                      title: 'KokonutUI AI Loading Stages',
                      desc: '@kokonutui/ai-loading — Orbital ring spinner with live rotating reasoning phase ticker.',
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
            )}

            {/* 11A. AGENT ACTIVITY (@beui/agent-activity) */}
            {activeSection === 'agent-activity' && (
              <div className="space-y-6">
                <div className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-border/80 bg-card p-3.5">
                  <div className="flex flex-wrap items-center gap-3">
                    <div className="flex items-center gap-1.5">
                      <span className="text-[11px] font-medium text-muted-foreground">Density</span>
                      <div className="inline-flex rounded-lg border border-border bg-muted/40 p-0.5 text-xs">
                        {ACTIVITY_DENSITY_OPTIONS.map((opt) => (
                          <button
                            key={opt.id}
                            type="button"
                            onClick={() => applyCustomization({ activityDensity: opt.id })}
                            className={cn(
                              'rounded-md px-2.5 py-1 text-[11px] font-medium transition-colors',
                              state.activityDensity === opt.id
                                ? 'bg-card text-foreground shadow-2xs'
                                : 'text-muted-foreground hover:text-foreground',
                            )}
                          >
                            {opt.title}
                          </button>
                        ))}
                      </div>
                    </div>

                    <div className="flex items-center gap-1.5">
                      <span className="text-[11px] font-medium text-muted-foreground">Viewport</span>
                      <div className="inline-flex rounded-lg border border-border bg-muted/40 p-0.5 text-xs">
                        {ACTIVITY_MAX_HEIGHT_OPTIONS.map((opt) => (
                          <button
                            key={opt.id}
                            type="button"
                            onClick={() => applyCustomization({ activityMaxHeight: opt.id })}
                            className={cn(
                              'rounded-md px-2.5 py-1 text-[11px] font-medium transition-colors',
                              state.activityMaxHeight === opt.id
                                ? 'bg-card text-foreground shadow-2xs'
                                : 'text-muted-foreground hover:text-foreground',
                            )}
                          >
                            {opt.shortLabel}
                          </button>
                        ))}
                      </div>
                    </div>
                  </div>

                  <div className="flex flex-wrap items-center gap-4 text-xs">
                    <label className="flex cursor-pointer items-center gap-2">
                      <span className="text-[11px] font-medium text-muted-foreground">
                        Auto-collapse
                      </span>
                      <Switch
                        checked={state.activityAutoCollapse}
                        onCheckedChange={(checked) =>
                          applyCustomization({ activityAutoCollapse: Boolean(checked) })
                        }
                      />
                    </label>
                    <label className="flex cursor-pointer items-center gap-2">
                      <span className="text-[11px] font-medium text-muted-foreground">
                        Step details
                      </span>
                      <Switch
                        checked={state.activityShowDetails}
                        onCheckedChange={(checked) =>
                          applyCustomization({ activityShowDetails: Boolean(checked) })
                        }
                      />
                    </label>
                  </div>
                </div>

                <div className="grid grid-cols-1 gap-6">
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
                        isActive
                        mode={opt.id === 'orb-card' ? 'inline-collapsible' : 'timeline'}
                        defaultOpen
                        variant={opt.id}
                        orbStyle={state.activityOrbStyle}
                        density={state.activityDensity}
                        maxHeight="unlimited"
                        showDetails={state.activityShowDetails}
                        steps={[
                          {
                            id: `${opt.id}-1`,
                            kind: 'reasoning',
                            contentType: 'reasoning',
                            title: 'Evaluating workspace changes',
                            detail: 'Checking registered tools and active skill bundle',
                            status: 'info',
                            timestamp: STUDIO_PREVIEW_ANCHOR_TS - 2400,
                          },
                          {
                            id: `${opt.id}-2`,
                            kind: 'search',
                            contentType: 'search',
                            title: 'semantic_search',
                            detail: 'Matched 18 memory & codebase nodes',
                            status: 'success',
                            durationMs: 64,
                            durationLabel: '64ms',
                            timestamp: STUDIO_PREVIEW_ANCHOR_TS - 1400,
                          },
                          {
                            id: `${opt.id}-3`,
                            kind: 'tool',
                            contentType: 'tool',
                            title: 'cargo_check',
                            detail: 'Type-checking workspace crates',
                            actor: 'builder',
                            status: 'running',
                            durationMs: 420,
                            durationLabel: '420ms',
                            timestamp: STUDIO_PREVIEW_ANCHOR_TS - 400,
                          },
                        ]}
                      />
                    </VariantPreviewCard>
                  ))}
                </div>
              </div>
            )}

            {/* 11B. ACTIVITY ORB (@beui/activity-orb) */}
            {activeSection === 'activity-orb' && (
              <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
                {ACTIVITY_ORB_OPTIONS.map((opt) => (
                  <VariantPreviewCard
                    key={opt.id}
                    title={opt.title}
                    description={opt.description}
                    selected={state.activityOrbStyle === opt.id}
                    isDefault={Boolean(opt.isDefault)}
                    onSelect={() => applyCustomization({ activityOrbStyle: opt.id })}
                  >
                    <div className="space-y-3">
                      <div className="flex items-center justify-between gap-2 rounded-xl border border-border/65 bg-card px-3.5 py-2.5 text-xs">
                        <div className="flex items-center gap-2 font-medium text-foreground">
                          <BeuiActivityOrb
                            phase="tool"
                            isActive
                            styleVariant={opt.id}
                            size="sm"
                          />
                          <span>Working through it…</span>
                        </div>
                        <span className="font-mono text-[10px] text-muted-foreground">3 steps</span>
                      </div>

                      <div className="flex items-center justify-around rounded-xl border border-border/60 bg-card/55 px-3 py-3">
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
                    </div>
                  </VariantPreviewCard>
                ))}
              </div>
            )}

            {/* 11C. ACTIVITY PLACEMENT (@beui/activity-placement) */}
            {activeSection === 'activity-placement' && (
              <div className="space-y-6">
                <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
                  {ACTIVITY_PLACEMENT_OPTIONS.map((opt) => {
                    const showInline = opt.id !== 'panel-only';
                    const collapsesOnDone =
                      opt.id === 'beui-timeline' || opt.id === 'streaming-only';
                    return (
                      <VariantPreviewCard
                        key={opt.id}
                        title={opt.title}
                        description={opt.description}
                        selected={state.activityStyle === opt.id}
                        isDefault={Boolean(opt.isDefault)}
                        onSelect={() => applyCustomization({ activityStyle: opt.id })}
                      >
                        <div className="grid grid-cols-[1fr_104px] gap-2.5 rounded-xl border border-border/70 bg-card/55 p-3 text-[11px]">
                          <div className="space-y-2 rounded-lg border border-border/60 bg-background/80 p-2.5">
                            <div className="flex items-center justify-between text-[10px] text-muted-foreground">
                              <span className="font-medium text-foreground">Chat Turn</span>
                              <span>
                                {opt.id === 'panel-only'
                                  ? 'Prose only'
                                  : opt.id === 'streaming-only'
                                    ? 'Live only'
                                    : collapsesOnDone
                                      ? 'Auto-folds'
                                      : 'Pinned'}
                              </span>
                            </div>
                            {showInline ? (
                              <div className="flex items-center justify-between rounded-md border border-primary/35 bg-primary/10 px-2 py-1 text-[10px] text-primary">
                                <span className="truncate font-medium">
                                  {opt.id === 'streaming-only'
                                    ? 'Live Activity (hides on reply)'
                                    : opt.id === 'beui-timeline'
                                      ? 'Completed 3 steps ▾'
                                      : 'Expanded Activity Card'}
                                </span>
                              </div>
                            ) : (
                              <div className="rounded-md border border-dashed border-border/60 px-2 py-1 text-[10px] text-muted-foreground">
                                No inline activity block
                              </div>
                            )}
                            <div className="h-2 w-4/5 rounded-full bg-foreground/20" />
                            <div className="h-2 w-3/5 rounded-full bg-foreground/15" />
                          </div>

                          <div
                            className={cn(
                              'flex flex-col justify-between rounded-lg border p-2.5',
                              opt.id === 'panel-only'
                                ? 'border-primary/45 bg-primary/10 text-primary'
                                : 'border-border/60 bg-background/65 text-muted-foreground',
                            )}
                          >
                            <div className="flex items-center gap-1 text-[10px] font-semibold">
                              <PanelRight className="size-3 shrink-0" />
                              <span className="truncate">Drawer</span>
                            </div>
                            <div className="space-y-1">
                              <div className="h-1.5 w-full rounded-full bg-current opacity-35" />
                              <div className="h-1.5 w-4/5 rounded-full bg-current opacity-25" />
                              <div className="h-1.5 w-3/5 rounded-full bg-current opacity-20" />
                            </div>
                            <span className="font-mono text-[9.5px]">Always live</span>
                          </div>
                        </div>
                      </VariantPreviewCard>
                    );
                  })}
                </div>
              </div>
            )}

            {/* 11D. ANIMATED TOAST STACK (@beui/animated-toast-stack) */}
            {activeSection === 'animated-toast-stack' && (
              <div className="space-y-6">
                <div className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-border/80 bg-card p-3.5">
                  <div>
                    <div className="text-xs font-semibold text-foreground">
                      Live Popup Toasts
                    </div>
                    <p className="text-xs text-muted-foreground">
                      Hover any preview stack below to fan out layered cards, or trigger a live toast on screen.
                    </p>
                  </div>

                  <div className="flex flex-wrap items-center gap-2">
                    <Button
                      type="button"
                      size="sm"
                      variant="secondary"
                      onClick={() => {
                        if (!state.toastEnabled) {
                          applyCustomization({ toastEnabled: true });
                        }
                        useToastStore.getState().pushToast({
                          dedupeKey: `demo-toast-${Date.now()}`,
                          category: 'skill',
                          status: 'info',
                          title: 'Skill used',
                          detail: 'openz_fullstack_playbook · matched active task',
                          actionLabel: 'Open Skills',
                          onAction: () => {
                            useOpenZStore.getState().setActiveView('skills');
                            setIsOpen(false);
                          },
                        });
                      }}
                    >
                      <span>Send Test Toast</span>
                    </Button>

                    <Button
                      type="button"
                      size="sm"
                      variant="outline"
                      onClick={() => {
                        if (!state.toastEnabled) {
                          applyCustomization({ toastEnabled: true });
                        }
                        const samples = [
                          {
                            category: 'workflow' as const,
                            status: 'info' as const,
                            title: 'Workflow matched',
                            detail: 'verified-webui-release',
                          },
                          {
                            category: 'skill' as const,
                            status: 'info' as const,
                            title: 'Skill used',
                            detail: 'rust_pro',
                          },
                          {
                            category: 'self_improvement' as const,
                            status: 'success' as const,
                            title: 'Self-improvement updated',
                            detail: 'Consolidated learnings into MEMORY.md',
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
                      <span>Test 3-Card Stack</span>
                    </Button>

                    <Button
                      type="button"
                      size="sm"
                      variant="ghost"
                      onClick={() => useToastStore.getState().clearActiveToasts()}
                    >
                      <span>Clear</span>
                    </Button>

                    <div className="ml-1 flex items-center gap-2 border-l border-border/65 pl-3">
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
                </div>

                <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
                  {TOAST_VARIANT_OPTIONS.map((opt) => (
                    <VariantPreviewCard
                      key={opt.id}
                      title={opt.title}
                      description={opt.description}
                      selected={state.toastVariant === opt.id}
                      isDefault={Boolean(opt.isDefault)}
                      onSelect={() => applyCustomization({ toastVariant: opt.id })}
                    >
                      <AnimatedToastStack
                        previewStatic
                        variant={opt.id}
                        maxVisible={3}
                        onDismiss={() => {}}
                        className="min-h-[136px] py-1"
                        toasts={[
                          {
                            id: `${opt.id}-preview-1`,
                            category: 'skill',
                            status: 'info',
                            title: 'Skill used',
                            detail: 'openz_fullstack_playbook',
                            createdAt: STUDIO_PREVIEW_ANCHOR_TS - 1000,
                            actionLabel: 'Open Skills',
                            onAction: () => {},
                          },
                          {
                            id: `${opt.id}-preview-2`,
                            category: 'self_improvement',
                            status: 'success',
                            title: 'Self-improvement updated',
                            detail: 'Consolidated session memory',
                            createdAt: STUDIO_PREVIEW_ANCHOR_TS - 4000,
                          },
                        ]}
                      />
                    </VariantPreviewCard>
                  ))}
                </div>
              </div>
            )}

            {/* 11E. TOAST PLACEMENT (@beui/toast-placement) */}
            {activeSection === 'toast-placement' && (
              <div className="space-y-6">
                <div className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-border/80 bg-card p-3.5">
                  <div className="flex flex-wrap items-center gap-3">
                    <div className="flex items-center gap-1.5">
                      <span className="text-[11px] font-medium text-muted-foreground">
                        Auto-Dismiss
                      </span>
                      <div className="inline-flex rounded-lg border border-border bg-muted/40 p-0.5 text-xs">
                        {TOAST_DURATION_OPTIONS.map((opt) => (
                          <button
                            key={opt.id}
                            type="button"
                            onClick={() => applyCustomization({ toastDuration: opt.id })}
                            className={cn(
                              'rounded-md px-2.5 py-1 text-[11px] font-medium transition-colors',
                              state.toastDuration === opt.id
                                ? 'bg-card text-foreground shadow-2xs'
                                : 'text-muted-foreground hover:text-foreground',
                            )}
                          >
                            {opt.shortLabel}
                          </button>
                        ))}
                      </div>
                    </div>

                    <div className="flex items-center gap-1.5">
                      <span className="text-[11px] font-medium text-muted-foreground">
                        Max Stack
                      </span>
                      <div className="inline-flex rounded-lg border border-border bg-muted/40 p-0.5 text-xs">
                        {TOAST_MAX_VISIBLE_OPTIONS.map((opt) => (
                          <button
                            key={opt.id}
                            type="button"
                            onClick={() => applyCustomization({ toastMaxVisible: opt.id })}
                            className={cn(
                              'rounded-md px-2.5 py-1 text-[11px] font-medium transition-colors',
                              state.toastMaxVisible === opt.id
                                ? 'bg-card text-foreground shadow-2xs'
                                : 'text-muted-foreground hover:text-foreground',
                            )}
                          >
                            {opt.shortLabel} cards
                          </button>
                        ))}
                      </div>
                    </div>
                  </div>

                  <Button
                    type="button"
                    size="sm"
                    variant="secondary"
                    onClick={() => {
                      if (!state.toastEnabled) {
                        applyCustomization({ toastEnabled: true });
                      }
                      useToastStore.getState().pushToast({
                        dedupeKey: `demo-pos-${Date.now()}`,
                        category: 'workflow',
                        status: 'info',
                        title: `Anchored at ${normalizeToastPosition(state.toastPosition)}`,
                        detail: 'Live toast stack placement preview',
                      });
                    }}
                  >
                    <span>Test Active Anchor</span>
                  </Button>
                </div>

                <div className="grid grid-cols-1 gap-6 sm:grid-cols-2 lg:grid-cols-3">
                  {TOAST_POSITION_OPTIONS.map((opt) => {
                    const activePos = normalizeToastPosition(state.toastPosition) === opt.id;
                    const anchorPosClass =
                      opt.id === 'top-start'
                        ? 'top-2.5 left-2.5'
                        : opt.id === 'top-center'
                          ? 'top-2.5 left-1/2 -translate-x-1/2'
                          : opt.id === 'top-end'
                            ? 'top-2.5 right-2.5'
                            : opt.id === 'bottom-start'
                              ? 'bottom-2.5 left-2.5'
                              : opt.id === 'bottom-center'
                                ? 'bottom-2.5 left-1/2 -translate-x-1/2'
                                : 'bottom-2.5 right-2.5';
                    return (
                      <VariantPreviewCard
                        key={opt.id}
                        title={opt.title}
                        description={opt.description}
                        selected={activePos}
                        isDefault={Boolean(opt.isDefault)}
                        onSelect={() => applyCustomization({ toastPosition: opt.id })}
                      >
                        <div className="relative h-28 w-full overflow-hidden rounded-xl border border-border/75 bg-card/65 p-3">
                          <div className="flex h-full items-center justify-center gap-2 opacity-35">
                            <div className="h-full w-8 rounded-md border border-border/60 bg-muted/40" />
                            <div className="flex h-full flex-1 flex-col justify-end gap-1.5 rounded-md border border-border/50 bg-background/50 p-2">
                              <div className="h-1.5 w-3/5 rounded-full bg-foreground/25" />
                              <div className="h-4 w-full rounded border border-border/60 bg-card/70" />
                            </div>
                          </div>

                          <div
                            className={cn(
                              'absolute flex w-28 flex-col gap-1 rounded-lg border border-primary/55 bg-primary/15 px-2.5 py-1.5 shadow-md backdrop-blur-xs',
                              anchorPosClass,
                            )}
                          >
                            <div className="flex items-center gap-1.5">
                              <span className="size-1.5 rounded-full bg-primary" />
                              <span className="truncate text-[10px] font-semibold text-foreground">
                                {opt.shortLabel}
                              </span>
                            </div>
                            <div className="h-1 w-3/4 rounded-full bg-primary/45" />
                          </div>
                        </div>
                      </VariantPreviewCard>
                    );
                  })}
                </div>
              </div>
            )}

            {/* 11F. TELEMETRY BADGES (@beui/telemetry-strip) */}
            {activeSection === 'telemetry-badges' && (
              <div className="space-y-6">
                <div className="grid grid-cols-1 gap-6">
                  {INLINE_NOTICE_STYLE_OPTIONS.map((opt) => (
                    <VariantPreviewCard
                      key={opt.id}
                      title={opt.title}
                      description={opt.description}
                      selected={state.inlineNoticeStyle === opt.id}
                      isDefault={Boolean(opt.isDefault)}
                      onSelect={() => applyCustomization({ inlineNoticeStyle: opt.id })}
                    >
                      <div className="space-y-2.5 rounded-xl border border-border/65 bg-card/65 p-3.5 text-xs">
                        {opt.id === 'toast-only' ? (
                          <div className="rounded-lg border border-dashed border-border/70 px-3 py-2 text-center text-[11px] text-muted-foreground">
                            Inline badges hidden — telemetry routed exclusively to Toast Stack &amp; Activity Drawer
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
                                timestamp: STUDIO_PREVIEW_ANCHOR_TS - 1500,
                              },
                              {
                                id: `preview-inline-si-${opt.id}`,
                                kind: 'self_improvement',
                                title: 'Self-improvement',
                                detail: 'Consolidated learnings into MEMORY.md',
                                timestamp: STUDIO_PREVIEW_ANCHOR_TS - 800,
                              },
                            ]}
                          />
                        )}
                        <div className="text-foreground/90">
                          Verified workspace checks and updated skill guidance cleanly.
                        </div>
                      </div>
                    </VariantPreviewCard>
                  ))}
                </div>

                <div className="rounded-xl border border-border/80 bg-card p-4">
                  <div className="mb-3">
                    <div className="text-xs font-semibold text-foreground">
                      Event Category Triggers
                    </div>
                    <p className="text-xs text-muted-foreground">
                      Choose which runtime events emit popup toasts. Muted categories still log quietly in Notifications.
                    </p>
                  </div>
                  <div className="grid grid-cols-1 gap-2.5 md:grid-cols-2">
                    {TOAST_CATEGORY_TOGGLE_OPTIONS.map((toggle) => (
                      <label
                        key={toggle.key}
                        className="flex cursor-pointer items-center justify-between gap-4 rounded-xl border border-border/60 bg-background/50 px-3.5 py-2.5"
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
                    {
                      id: 'rare-code',
                      title: 'Rare-UI Window Code Block',
                      desc: 'swamimalode07/rare-ui/code-block — Rounded studio window chrome with subtle header dots.',
                      isDefault: false,
                    },
                    {
                      id: 'beautiful-code',
                      title: 'BeautifulUI Streaming Code Block',
                      desc: 'beautifului.dev/r/code-block — Clean hairline code surface optimized for live token streaming.',
                      isDefault: false,
                    },
                    {
                      id: 'joly-code',
                      title: 'JolyUI Tabbed Code Block',
                      desc: 'jolyui.dev/r/code-block — High-contrast background surface with tinted header strip.',
                      isDefault: false,
                    },
                    {
                      id: 'aicss-code',
                      title: 'AICSS Agent Code Block',
                      desc: 'aicss.dev/r/code-block — Subtle primary-accented code frame for agentic coding.',
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
                    {
                      id: 'assistant-code-diff',
                      title: 'Assistant-UI Code Diff',
                      desc: 'assistant-ui/elements/code-diff — Chat-optimized tinted additions & removals.',
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
                    {
                      id: 'assistant-terminal-block',
                      title: 'Assistant-UI Terminal Block',
                      desc: 'assistant-ui/elements/terminal-block — Streaming line-by-line shell block with exit status.',
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
                    {
                      id: 'assistant-image-gen',
                      title: 'Assistant-UI Image Generation',
                      desc: 'assistant-ui/elements/image-generation — Framed gallery surface with caption footer.',
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
                    {
                      id: 'kokonut-citations',
                      title: 'KokonutUI Citation Chips',
                      desc: '@kokonutui/ai-citations — Monochrome hairline host chips with numbered indices.',
                      isDefault: false,
                    },
                    {
                      id: 'minimal-inline',
                      title: 'Minimal Domain Links',
                      desc: 'Borderless single-row domain references.',
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
                    {
                      id: 'uiarc-inspector',
                      title: 'UIArc JSON Inspector',
                      desc: '@uiarc/json-viewer — Searchable JSON inspector with live key/value filter and primitive type tags.',
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

            {/* 23A. ANIMATED ALERT (@beui/alert) */}
            {activeSection === 'alert-collapsible' && (
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
            )}

            {/* 23B. COLLAPSIBLE (@beui/collapsible) */}
            {activeSection === 'collapsible' && (
              <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
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
            )}

            <ExtendedElementsGalleries
              activeSection={activeSection}
              state={state}
              applyCustomization={applyCustomization}
            />
          </div>
        </main>
      </div>
    </div>
  );
};
