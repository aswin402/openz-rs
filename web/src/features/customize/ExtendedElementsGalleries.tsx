import React, { useState } from 'react';
import {
  Activity,
  Bot,
  MessageSquare,
  Palette,
  Settings,
} from 'lucide-react';
import {
  AssistantLoadingStateElement,
  ClassicThemeToggleIcon,
  DOCK_TOOLBAR_STYLE_OPTIONS,
  DRAWER_MOTION_STYLE_OPTIONS,
  KokonutTokenAlternatives,
  KokonutToolbarDock,
  LOADING_STATE_STYLE_OPTIONS,
  MEMORY_CHIPS_STYLE_OPTIONS,
  MemoryChipsElement,
  NAV_MENU_STYLE_OPTIONS,
  NOTIFICATION_CENTER_STYLE_OPTIONS,
  RareNotificationBell,
  SpectrumNavListCard,
  SpectrumStatusBadge,
  TOKEN_ALTERNATIVES_STYLE_OPTIONS,
} from '../../components/registries/modernRegistries';
import {
  AGENT_HANDOFF_STYLE_OPTIONS,
  ARTIFACT_CARD_STYLE_OPTIONS,
  AgentHandoffElement,
  ArtifactCardElement,
  BACKGROUND_INBOX_STYLE_OPTIONS,
  BackgroundInboxElement,
  ERROR_STATE_STYLE_OPTIONS,
  ErrorStateElement,
  FILE_TREE_STYLE_OPTIONS,
  FileTreeElement,
  MATH_BLOCK_STYLE_OPTIONS,
  MathBlockElement,
  RECOMMENDATION_STYLE_OPTIONS,
  RESEARCH_REPORT_STYLE_OPTIONS,
  RETRIEVAL_CHUNKS_STYLE_OPTIONS,
  RecommendationCardElement,
  ResearchReportElement,
  RetrievalChunksElement,
  SPEC_SHEET_STYLE_OPTIONS,
  SpecSheetElement,
  TIMELINE_STYLE_OPTIONS,
  TimelineElement,
  WEB_SEARCH_STYLE_OPTIONS,
  WebSearchElement,
} from '../../components/assistant-ui/elements';
import {
  COMPARISON_STYLE_OPTIONS,
  ComparisonTableElement,
  DELETE_BUTTON_STYLE_OPTIONS,
  INLINE_CITATION_STYLE_OPTIONS,
  InlineCitationLink,
  RareDeleteButton,
} from '../../components/registries/rareAndCommunity';
import { Badge, GLOBAL_BADGE_STYLE_OPTIONS } from '../../components/ui/badge';
import type {
  CustomizeUiSectionId,
  UiCustomizationSnapshot,
} from '../../store/useUiCustomizationStore';
import { VariantPreviewCard } from './VariantPreviewCard';

export interface ExtendedElementsGalleriesProps {
  activeSection: CustomizeUiSectionId;
  state: UiCustomizationSnapshot;
  applyCustomization: (patch: Partial<UiCustomizationSnapshot>) => void;
}

export const ExtendedElementsGalleries: React.FC<ExtendedElementsGalleriesProps> = ({
  activeSection,
  state,
  applyCustomization,
}) => {
  const [previewDockId, setPreviewDockId] = useState('chat');
  const [previewDark, setPreviewDark] = useState(true);
  const [previewNavId, setPreviewNavId] = useState('theme');
  const [previewCavemanOn, setPreviewCavemanOn] = useState(true);
  const [previewCavemanLevel, setPreviewCavemanLevel] = useState<'lite' | 'full' | 'ultra' | 'symbolic_full'>('full');

  switch (activeSection) {
    case 'dock-toolbar':
      return (
        <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
          {DOCK_TOOLBAR_STYLE_OPTIONS.map((opt) => (
            <VariantPreviewCard
              key={opt.id}
              title={opt.title}
              description={opt.description}
              selected={state.dockToolbarStyle === opt.id}
              isDefault={Boolean(opt.isDefault)}
              onSelect={() => applyCustomization({ dockToolbarStyle: opt.id })}
            >
              <div className="flex items-center justify-center py-2">
                <KokonutToolbarDock
                  variant={opt.id}
                  orientation="horizontal"
                  layoutGroupId={`preview-dock-${opt.id}`}
                  items={[
                    {
                      id: 'chat',
                      label: 'Chat',
                      active: previewDockId === 'chat',
                      onClick: () => setPreviewDockId('chat'),
                      renderIcon: () => <MessageSquare className="size-4" />,
                    },
                    {
                      id: 'agents',
                      label: 'Agents',
                      active: previewDockId === 'agents',
                      onClick: () => setPreviewDockId('agents'),
                      renderIcon: () => <Bot className="size-4" />,
                    },
                    {
                      id: 'notifications',
                      label: 'Alerts',
                      active: previewDockId === 'notifications',
                      onClick: () => setPreviewDockId('notifications'),
                      renderIcon: () => (
                        <RareNotificationBell
                          unreadCount={3}
                          active={previewDockId === 'notifications'}
                        />
                      ),
                      dividerBefore: true,
                    },
                    {
                      id: 'theme',
                      label: previewDark ? 'Dark' : 'Light',
                      active: false,
                      onClick: () => setPreviewDark((d) => !d),
                      renderIcon: () => <ClassicThemeToggleIcon isDark={previewDark} />,
                    },
                  ]}
                />
              </div>
            </VariantPreviewCard>
          ))}
        </div>
      );

    case 'drawer-motion':
      return (
        <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
          {DRAWER_MOTION_STYLE_OPTIONS.map((opt) => (
            <VariantPreviewCard
              key={opt.id}
              title={opt.title}
              description={opt.description}
              selected={state.drawerMotionStyle === opt.id}
              isDefault={Boolean(opt.isDefault)}
              onSelect={() => applyCustomization({ drawerMotionStyle: opt.id })}
            >
              <div className="flex items-center justify-between rounded-xl border border-border/65 bg-card/80 px-3.5 py-3 text-xs">
                <div className="space-y-0.5">
                  <div className="font-semibold text-foreground">Activity &amp; Logs Drawer</div>
                  <div className="font-mono text-[10px] text-muted-foreground">
                    {opt.id === 'kokonut-smooth'
                      ? 'spring(350, 32)'
                      : opt.id === 'snappy-slide'
                        ? 'cubic-out 180ms'
                        : '0ms instant'}
                  </div>
                </div>
                <SpectrumStatusBadge
                  tone={opt.id === 'kokonut-smooth' ? 'running' : 'online'}
                  label={opt.id}
                />
              </div>
            </VariantPreviewCard>
          ))}
        </div>
      );

    case 'nav-menu':
      return (
        <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
          {NAV_MENU_STYLE_OPTIONS.map((opt) => (
            <VariantPreviewCard
              key={opt.id}
              title={opt.title}
              description={opt.description}
              selected={state.navMenuStyle === opt.id}
              isDefault={Boolean(opt.isDefault)}
              onSelect={() => applyCustomization({ navMenuStyle: opt.id })}
            >
              <SpectrumNavListCard
                variant={opt.id}
                activeId={previewNavId}
                onSelect={setPreviewNavId}
                layoutGroupId={`preview-nav-${opt.id}`}
                items={[
                  {
                    id: 'theme',
                    label: 'Global Theme',
                    subtitle: '@beui/theme',
                    badge: '19',
                    icon: Palette,
                  },
                  {
                    id: 'activity',
                    label: 'Agent Activity',
                    subtitle: '@beui/agent-activity',
                    icon: Activity,
                  },
                  {
                    id: 'settings',
                    label: 'Gateway Config',
                    subtitle: 'openz.json',
                    icon: Settings,
                  },
                ]}
              />
            </VariantPreviewCard>
          ))}
        </div>
      );

    case 'token-alternatives':
      return (
        <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
          {TOKEN_ALTERNATIVES_STYLE_OPTIONS.map((opt) => (
            <VariantPreviewCard
              key={opt.id}
              title={opt.title}
              description={opt.description}
              selected={state.tokenAlternativesStyle === opt.id}
              isDefault={Boolean(opt.isDefault)}
              onSelect={() => applyCustomization({ tokenAlternativesStyle: opt.id })}
            >
              <div className="flex items-center justify-center rounded-xl border border-border/60 bg-card/70 p-4">
                <KokonutTokenAlternatives
                  enabled={previewCavemanOn}
                  activeLevel={previewCavemanLevel}
                  savingsBadge="✂411k"
                  variant={opt.id}
                  onToggleEnabled={() => setPreviewCavemanOn((v) => !v)}
                  onSelectLevel={(lvl) => setPreviewCavemanLevel(lvl as typeof previewCavemanLevel)}
                />
              </div>
            </VariantPreviewCard>
          ))}
        </div>
      );

    case 'loading-state':
      return (
        <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
          {LOADING_STATE_STYLE_OPTIONS.map((opt) => (
            <VariantPreviewCard
              key={opt.id}
              title={opt.title}
              description={opt.description}
              selected={state.loadingStateStyle === opt.id}
              isDefault={Boolean(opt.isDefault)}
              onSelect={() => applyCustomization({ loadingStateStyle: opt.id })}
            >
              <div className="rounded-xl border border-border/60 bg-card/70 p-4">
                <AssistantLoadingStateElement variant={opt.id} />
              </div>
            </VariantPreviewCard>
          ))}
        </div>
      );

    case 'notification-center':
      return (
        <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
          {NOTIFICATION_CENTER_STYLE_OPTIONS.map((opt) => (
            <VariantPreviewCard
              key={opt.id}
              title={opt.title}
              description={opt.description}
              selected={state.notificationCenterStyle === opt.id}
              isDefault={Boolean(opt.isDefault)}
              onSelect={() => applyCustomization({ notificationCenterStyle: opt.id })}
            >
              <div className="space-y-2 rounded-xl border border-border/65 bg-card/80 p-3 text-xs">
                <div className="flex items-center justify-between border-b border-border/50 pb-2">
                  <div className="flex items-center gap-2 font-semibold text-foreground">
                    <RareNotificationBell unreadCount={2} active />
                    <span>Notifications</span>
                  </div>
                  <SpectrumStatusBadge tone="running" label="2 new" />
                </div>
                {opt.id === 'uiarc-center' && (
                  <div className="flex gap-1 text-[10px]">
                    <span className="rounded-md bg-primary/15 px-2 py-0.5 font-semibold text-primary">
                      All
                    </span>
                    <span className="rounded-md px-2 py-0.5 text-muted-foreground">Alerts</span>
                    <span className="rounded-md px-2 py-0.5 text-muted-foreground">Skills</span>
                  </div>
                )}
                <div
                  className={
                    opt.id === 'tweenly-feed'
                      ? 'space-y-1.5 border-l-2 border-primary/40 pl-2.5'
                      : 'space-y-1.5'
                  }
                >
                  <div className="rounded-lg border border-border/55 bg-background/60 px-2.5 py-1.5">
                    <div className="font-medium text-foreground">Skill consolidated</div>
                    <div className="text-[10.5px] text-muted-foreground">rust-pro · 12s ago</div>
                  </div>
                </div>
              </div>
            </VariantPreviewCard>
          ))}
        </div>
      );

    case 'memory-chips':
      return (
        <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
          {MEMORY_CHIPS_STYLE_OPTIONS.map((opt) => (
            <VariantPreviewCard
              key={opt.id}
              title={opt.title}
              description={opt.description}
              selected={state.memoryChipsStyle === opt.id}
              isDefault={Boolean(opt.isDefault)}
              onSelect={() => applyCustomization({ memoryChipsStyle: opt.id })}
            >
              <div className="rounded-xl border border-border/60 bg-card/70 p-3.5">
                <MemoryChipsElement
                  variant={opt.id}
                  chips={[
                    {
                      id: 'm1',
                      action: 'recalled',
                      label: 'TurnState monotonic phase',
                    },
                    {
                      id: 'm2',
                      action: 'saved',
                      label: 'Floating composer dock',
                    },
                    {
                      id: 'm3',
                      action: 'source',
                      label: 'NVIDIA Q3 briefing',
                    },
                  ]}
                />
              </div>
            </VariantPreviewCard>
          ))}
        </div>
      );

    case 'badge-style':
      return (
        <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
          {GLOBAL_BADGE_STYLE_OPTIONS.map((opt) => (
            <VariantPreviewCard
              key={opt.id}
              title={opt.title}
              description={opt.description}
              selected={state.badgeStyle === opt.id}
              isDefault={Boolean(opt.isDefault)}
              onSelect={() => applyCustomization({ badgeStyle: opt.id })}
            >
              <div
                data-badge-style={opt.id}
                className="flex flex-wrap items-center justify-center gap-2 rounded-xl border border-border/60 bg-card/70 p-4"
              >
                <Badge variant="default">Active</Badge>
                <Badge variant="secondary">claude-sonnet-4</Badge>
                <Badge variant="outline">229 tools</Badge>
                <Badge variant="success">Verified</Badge>
                <Badge variant="warning">Approval</Badge>
              </div>
            </VariantPreviewCard>
          ))}
        </div>
      );

    case 'delete-button':
      return (
        <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
          {DELETE_BUTTON_STYLE_OPTIONS.map((opt) => (
            <VariantPreviewCard
              key={opt.id}
              title={opt.title}
              description={opt.description}
              selected={state.deleteButtonStyle === opt.id}
              isDefault={Boolean(opt.isDefault)}
              onSelect={() => applyCustomization({ deleteButtonStyle: opt.id })}
            >
              <div className="flex items-center justify-between rounded-xl border border-border/65 bg-card/75 px-3.5 py-2.5 text-xs">
                <span className="truncate font-medium text-foreground">
                  Session · Rust Orchestrator
                </span>
                <RareDeleteButton
                  variant={opt.id}
                  iconOnly={false}
                  label="Delete"
                  onDelete={() => {}}
                />
              </div>
            </VariantPreviewCard>
          ))}
        </div>
      );

    case 'recommendation-card':
      return (
        <div className="grid grid-cols-1 gap-6">
          {RECOMMENDATION_STYLE_OPTIONS.map((opt) => (
            <VariantPreviewCard
              key={opt.id}
              title={opt.title}
              description={opt.description}
              selected={state.recommendationStyle === opt.id}
              isDefault={Boolean(opt.isDefault)}
              onSelect={() => applyCustomization({ recommendationStyle: opt.id })}
            >
              <RecommendationCardElement
                variant={opt.id}
                items={[
                  {
                    id: 'rec-1',
                    title: 'Run focused package check',
                    rationale: 'Verify zero warnings across changed Rust modules before release.',
                    actionLabel: 'Run',
                    onSelect: () => {},
                  },
                  {
                    id: 'rec-2',
                    title: 'Consolidate turn into SKILL.md',
                    rationale: 'Save reusable workflow steps for future sessions.',
                    actionLabel: 'Save',
                    onSelect: () => {},
                  },
                ]}
              />
            </VariantPreviewCard>
          ))}
        </div>
      );

    case 'agent-handoff':
      return (
        <div className="grid grid-cols-1 gap-6">
          {AGENT_HANDOFF_STYLE_OPTIONS.map((opt) => (
            <VariantPreviewCard
              key={opt.id}
              title={opt.title}
              description={opt.description}
              selected={state.agentHandoffStyle === opt.id}
              isDefault={Boolean(opt.isDefault)}
              onSelect={() => applyCustomization({ agentHandoffStyle: opt.id })}
            >
              <AgentHandoffElement
                variant={opt.id}
                handoffs={[
                  {
                    id: 'ho-1',
                    fromAgent: 'openz',
                    toAgent: 'code_auditor',
                    taskSummary: 'Forked isolated skill verification across workspace modules',
                    status: 'completed',
                  },
                  {
                    id: 'ho-2',
                    fromAgent: 'code_auditor',
                    toAgent: 'reviewer',
                    taskSummary: 'Validating security guard & zero-regex invariants',
                    status: 'running',
                  },
                ]}
              />
            </VariantPreviewCard>
          ))}
        </div>
      );

    case 'timeline':
      return (
        <div className="grid grid-cols-1 gap-6">
          {TIMELINE_STYLE_OPTIONS.map((opt) => (
            <VariantPreviewCard
              key={opt.id}
              title={opt.title}
              description={opt.description}
              selected={state.timelineStyle === opt.id}
              isDefault={Boolean(opt.isDefault)}
              onSelect={() => applyCustomization({ timelineStyle: opt.id })}
            >
              <TimelineElement
                variant={opt.id}
                items={[
                  {
                    id: 'tl-1',
                    title: 'Session restored & skills pinned',
                    detail: 'Loaded SKILL.md catalog with 100% KV-cache stability',
                    timeLabel: '0.1s',
                    status: 'completed',
                  },
                  {
                    id: 'tl-2',
                    title: 'Parallel codebase verification',
                    detail: 'Running focused type-check and unit suite',
                    timeLabel: '1.4s',
                    status: 'active',
                  },
                  {
                    id: 'tl-3',
                    title: 'Post-turn curator consolidation',
                    timeLabel: 'queued',
                    status: 'upcoming',
                  },
                ]}
              />
            </VariantPreviewCard>
          ))}
        </div>
      );

    case 'background-inbox':
      return (
        <div className="grid grid-cols-1 gap-6">
          {BACKGROUND_INBOX_STYLE_OPTIONS.map((opt) => (
            <VariantPreviewCard
              key={opt.id}
              title={opt.title}
              description={opt.description}
              selected={state.backgroundInboxStyle === opt.id}
              isDefault={Boolean(opt.isDefault)}
              onSelect={() => applyCustomization({ backgroundInboxStyle: opt.id })}
            >
              <BackgroundInboxElement
                variant={opt.id}
                tasks={[
                  {
                    id: 'bg-1',
                    title: 'Self-improvement curator',
                    detail: 'Reviewing trajectory for skill refinement',
                    status: 'running',
                    timeLabel: 'now',
                  },
                  {
                    id: 'bg-2',
                    title: 'Cron · Workspace health check',
                    detail: 'Verified ~/.openz SQLite WAL & memory graph',
                    status: 'completed',
                    timeLabel: '2m ago',
                  },
                ]}
              />
            </VariantPreviewCard>
          ))}
        </div>
      );

    case 'error-state':
      return (
        <div className="grid grid-cols-1 gap-6">
          {ERROR_STATE_STYLE_OPTIONS.map((opt) => (
            <VariantPreviewCard
              key={opt.id}
              title={opt.title}
              description={opt.description}
              selected={state.errorStateStyle === opt.id}
              isDefault={Boolean(opt.isDefault)}
              onSelect={() => applyCustomization({ errorStateStyle: opt.id })}
            >
              <ErrorStateElement
                message={`⚠️ **Error**: Provider 'google_ai_studio' error (HTTP 429): All retries exhausted (3 attempts). Last error: [{"error":{"code":429,"message":"You exceeded your current quota, please check your plan and billing details.\\nPlease retry in 21h42m14.363s.","status":"RESOURCE_EXHAUSTED","details":[{"retryDelay":"78134s","violations":[{"quotaDimensions":{"model":"gemini-2.5-flash"}}]}]}}]`}
                onRetry={() => {}}
                onOpenSettings={() => {}}
                variant={opt.id}
              />
            </VariantPreviewCard>
          ))}
        </div>
      );

    case 'file-tree':
      return (
        <div className="grid grid-cols-1 gap-6">
          {FILE_TREE_STYLE_OPTIONS.map((opt) => (
            <VariantPreviewCard
              key={opt.id}
              title={opt.title}
              description={opt.description}
              selected={state.fileTreeStyle === opt.id}
              isDefault={Boolean(opt.isDefault)}
              onSelect={() => applyCustomization({ fileTreeStyle: opt.id })}
            >
              <FileTreeElement
                defaultOpen
                variant={opt.id}
                entries={[
                  { path: 'src/agent/skills.rs', action: 'modified', additions: 42, deletions: 8 },
                  { path: 'web/src/components/ui/badge.tsx', action: 'modified', additions: 28, deletions: 5 },
                  { path: 'web/src/components/registries/thinkingOrbs.tsx', action: 'created', additions: 164 },
                ]}
              />
            </VariantPreviewCard>
          ))}
        </div>
      );

    case 'artifact-card':
      return (
        <div className="grid grid-cols-1 gap-6">
          {ARTIFACT_CARD_STYLE_OPTIONS.map((opt) => (
            <VariantPreviewCard
              key={opt.id}
              title={opt.title}
              description={opt.description}
              selected={state.artifactCardStyle === opt.id}
              isDefault={Boolean(opt.isDefault)}
              onSelect={() => applyCustomization({ artifactCardStyle: opt.id })}
            >
              <ArtifactCardElement
                variant={opt.id}
                artifact={{
                  id: 'art-1',
                  title: 'architecture-audit.md',
                  path: 'docs/architecture-audit.md',
                  kind: 'markdown',
                  lineCount: 84,
                  preview: '# Architecture Audit\n\n- Zero-regex guard verified across 100% of crates\n- 229 native tools registered cleanly',
                }}
              />
            </VariantPreviewCard>
          ))}
        </div>
      );

    case 'web-search':
      return (
        <div className="grid grid-cols-1 gap-6">
          {WEB_SEARCH_STYLE_OPTIONS.map((opt) => (
            <VariantPreviewCard
              key={opt.id}
              title={opt.title}
              description={opt.description}
              selected={state.webSearchStyle === opt.id}
              isDefault={Boolean(opt.isDefault)}
              onSelect={() => applyCustomization({ webSearchStyle: opt.id })}
            >
              <WebSearchElement
                defaultOpen
                variant={opt.id}
                data={{
                  id: 'ws-demo',
                  query: 'tokio async runtime work-stealing scheduler',
                  running: false,
                  results: [
                    {
                      title: 'Tokio Runtime Scheduler Architecture',
                      url: 'https://tokio.rs/blog/2019-10-scheduler',
                      snippet: 'Multi-threaded work-stealing scheduler with lock-free run queues.',
                    },
                    {
                      title: 'Assistant-UI Elements Registry',
                      url: 'https://assistant-ui.com/elements',
                      snippet: 'Composable agentic interface primitives built on shadcn/ui.',
                    },
                  ],
                }}
              />
            </VariantPreviewCard>
          ))}
        </div>
      );

    case 'retrieval-chunks':
      return (
        <div className="grid grid-cols-1 gap-6">
          {RETRIEVAL_CHUNKS_STYLE_OPTIONS.map((opt) => (
            <VariantPreviewCard
              key={opt.id}
              title={opt.title}
              description={opt.description}
              selected={state.retrievalChunksStyle === opt.id}
              isDefault={Boolean(opt.isDefault)}
              onSelect={() => applyCustomization({ retrievalChunksStyle: opt.id })}
            >
              <RetrievalChunksElement
                defaultOpen
                variant={opt.id}
                chunks={[
                  {
                    id: 'rc-1',
                    source: 'src/agent/agent_loop/mod.rs',
                    score: 0.94,
                    snippet: 'TurnState transitions deterministically across Restore → Compact → Command → Build → Run → Save → Respond.',
                  },
                  {
                    id: 'rc-2',
                    source: '~/.openz/skills/rust_pro/SKILL.md',
                    score: 0.88,
                    snippet: 'Prefer pattern-matching dispatch tables over imperative branching.',
                  },
                ]}
              />
            </VariantPreviewCard>
          ))}
        </div>
      );

    case 'research-report':
      return (
        <div className="grid grid-cols-1 gap-6">
          {RESEARCH_REPORT_STYLE_OPTIONS.map((opt) => (
            <VariantPreviewCard
              key={opt.id}
              title={opt.title}
              description={opt.description}
              selected={state.researchReportStyle === opt.id}
              isDefault={Boolean(opt.isDefault)}
              onSelect={() => applyCustomization({ researchReportStyle: opt.id })}
            >
              <ResearchReportElement
                title="Autonomous Skill Systems Field Comparison"
                variant={opt.id}
                sections={[
                  {
                    id: 'rs-1',
                    heading: 'Filesystem Single Source of Truth',
                    summary: 'Keeping SKILL.md authoritative on disk prevents stale database zombie resurrections.',
                    sourcesCount: 3,
                  },
                  {
                    id: 'rs-2',
                    heading: 'Subagent Fork Isolation',
                    summary: 'Delegating heavy audits via context: fork preserves parent KV-cache prefixes.',
                    sourcesCount: 2,
                  },
                ]}
              />
            </VariantPreviewCard>
          ))}
        </div>
      );

    case 'inline-citations':
      return (
        <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
          {INLINE_CITATION_STYLE_OPTIONS.map((opt) => (
            <VariantPreviewCard
              key={opt.id}
              title={opt.title}
              description={opt.description}
              selected={state.inlineCitationStyle === opt.id}
              isDefault={Boolean(opt.isDefault)}
              onSelect={() => applyCustomization({ inlineCitationStyle: opt.id })}
            >
              <div className="rounded-xl border border-border/65 bg-card/75 p-3.5 text-xs leading-relaxed text-foreground/90">
                OpenZ coordinates async tasks on{' '}
                <InlineCitationLink
                  href="https://tokio.rs"
                  index={1}
                  variant={opt.id}
                >
                  tokio.rs
                </InlineCitationLink>{' '}
                with composable UI elements from{' '}
                <InlineCitationLink
                  href="https://assistant-ui.com"
                  index={2}
                  variant={opt.id}
                >
                  assistant-ui
                </InlineCitationLink>
                .
              </div>
            </VariantPreviewCard>
          ))}
        </div>
      );

    case 'math-block':
      return (
        <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
          {MATH_BLOCK_STYLE_OPTIONS.map((opt) => (
            <VariantPreviewCard
              key={opt.id}
              title={opt.title}
              description={opt.description}
              selected={state.mathBlockStyle === opt.id}
              isDefault={Boolean(opt.isDefault)}
              onSelect={() => applyCustomization({ mathBlockStyle: opt.id })}
            >
              <MathBlockElement
                latex="\mathcal{L}(\theta) = \mathbb{E}_{x \sim \mathcal{D}}\big[-\log p_\theta(x)\big]"
                variant={opt.id}
              >
                <span className="font-serif text-sm italic text-foreground">
                  ℒ(θ) = 𝔼[−log p_θ(x)]
                </span>
              </MathBlockElement>
            </VariantPreviewCard>
          ))}
        </div>
      );

    case 'spec-sheet':
      return (
        <div className="grid grid-cols-1 gap-6">
          {SPEC_SHEET_STYLE_OPTIONS.map((opt) => (
            <VariantPreviewCard
              key={opt.id}
              title={opt.title}
              description={opt.description}
              selected={state.specSheetStyle === opt.id}
              isDefault={Boolean(opt.isDefault)}
              onSelect={() => applyCustomization({ specSheetStyle: opt.id })}
            >
              <SpecSheetElement
                title="OpenZ Runtime Profile"
                variant={opt.id}
                specs={[
                  { label: 'Async Runtime', value: 'Tokio Multi-Thread' },
                  { label: 'Native Tools', value: '229 in-process' },
                  { label: 'Vector Memory', value: 'SQLite WAL + FastEmbed' },
                ]}
              />
            </VariantPreviewCard>
          ))}
        </div>
      );

    case 'comparison-card':
      return (
        <div className="grid grid-cols-1 gap-6">
          {COMPARISON_STYLE_OPTIONS.map((opt) => (
            <VariantPreviewCard
              key={opt.id}
              title={opt.title}
              description={opt.description}
              selected={state.comparisonStyle === opt.id}
              isDefault={Boolean(opt.isDefault)}
              onSelect={() => applyCustomization({ comparisonStyle: opt.id })}
            >
              <ComparisonTableElement
                variant={opt.id}
                headers={['Feature', 'Native Rust Tool', 'Stdio MCP Process']}
                rows={[
                  ['Cold Start Latency', '0.2ms', '180ms'],
                  ['Zero Subprocess Spawn', 'Yes', 'No'],
                  ['Typed Schema Validation', 'Yes', 'Yes'],
                ]}
              />
            </VariantPreviewCard>
          ))}
        </div>
      );

    default:
      return null;
  }
};
