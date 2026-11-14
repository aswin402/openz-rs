import React, { useMemo, useState } from 'react';
import {
  BookOpen,
  CheckCircle2,
  Gauge,
  Languages,
  ShieldCheck,
  Sparkles,
  Zap,
} from 'lucide-react';
import { useOpenZStore } from '../../store/useOpenZStore';
import type { AgentDefaultsConfig } from '../../types';
import {
  CAVEMAN_LEVEL_OPTIONS,
  formatCompactTokenCount,
  getCavemanLevelOption,
  normalizeCavemanLevel,
  type CavemanLevelSlug,
  type CavemanScriptFamily,
} from '../../shared/lib/cavemanLevels';
import type { SettingsForm } from './settingsSchema';
import { Badge } from '../../components/ui/badge';
import { Switch } from '../../components/ui/switch';
import { cn } from '../../lib/utils';

type ScriptFilterId = 'all' | CavemanScriptFamily;

interface ScriptFilterOption {
  id: ScriptFilterId;
  label: string;
  badge: string;
}

const SCRIPT_FILTER_OPTIONS: readonly ScriptFilterOption[] = [
  { id: 'all', label: 'All Modes', badge: '6' },
  { id: 'latin_controlled', label: 'English ASD-STE100', badge: '3' },
  { id: 'classical_wenyan', label: 'Classical Wenyan 文言', badge: '3' },
];

const SCRIPT_FAMILY_LABELS: Readonly<Record<CavemanScriptFamily, string>> = {
  latin_controlled: 'ASD-STE100 English',
  classical_wenyan: 'Classical Wenyan 文言',
};

const AUTO_CLARITY_GUARDRAILS: readonly { title: string; detail: string }[] = [
  {
    title: 'Security & Credentials',
    detail: 'Full precision restored automatically for auth, secrets, CVEs, and SecurityGuard prompts.',
  },
  {
    title: 'Irreversible Operations',
    detail: 'Destructive filesystem, database, deployment, and production git steps stay explicit.',
  },
  {
    title: 'Verbatim Code & Paths',
    detail: 'Code blocks, CLI flags, file paths, identifiers, and JSON schemas are never altered.',
  },
  {
    title: 'Multi-Step Onboarding & Compliance',
    detail: 'Sensitive legal, medical, financial, and ordered safety checklists bypass compression.',
  },
];

const BUILTIN_CAVEMAN_SKILLS: readonly { slug: string; summary: string }[] = [
  {
    slug: 'caveman',
    summary: 'Core controlled-language response compression & telemetry skill.',
  },
  {
    slug: 'caveman_commit',
    summary: 'Conventional Commits formatter (<= 50 char imperative subject, signal-only body).',
  },
  {
    slug: 'caveman_review',
    summary: 'Single-line L<line>: <severity> <problem>. <fix>. code review protocol.',
  },
  {
    slug: 'caveman_compress',
    summary: 'Lossless workspace Markdown/memory compactor preserving sibling .orig.md backups.',
  },
];

export interface CavemanSettingsTabProps {
  settings: AgentDefaultsConfig | null;
  form: SettingsForm;
  onFieldChange: (key: string, value: string | number | boolean) => void;
}

export const CavemanSettingsTab: React.FC<CavemanSettingsTabProps> = ({
  settings,
  form,
  onFieldChange,
}) => {
  const storeCavemanMode = useOpenZStore((s) => s.cavemanMode);
  const storeCavemanLevel = useOpenZStore((s) => s.cavemanLevel);
  const storeCavemanStats = useOpenZStore((s) => s.cavemanStats);
  const updateSettings = useOpenZStore((s) => s.updateSettings);

  const [scriptFilter, setScriptFilter] = useState<ScriptFilterId>('all');

  const isEnabled =
    typeof form.caveman_mode === 'boolean'
      ? form.caveman_mode
      : typeof settings?.caveman_mode === 'boolean'
        ? settings.caveman_mode
        : storeCavemanMode;

  const activeLevel = normalizeCavemanLevel(
    form.caveman_level ?? settings?.caveman_level ?? storeCavemanLevel,
  );
  const activeOption = getCavemanLevelOption(activeLevel);
  const stats = storeCavemanStats ?? settings?.caveman_stats ?? null;

  const visibleModes = useMemo(
    () =>
      scriptFilter === 'all'
        ? CAVEMAN_LEVEL_OPTIONS
        : CAVEMAN_LEVEL_OPTIONS.filter((opt) => opt.scriptFamily === scriptFilter),
    [scriptFilter],
  );

  const handleToggleMode = (nextEnabled: boolean) => {
    onFieldChange('caveman_mode', nextEnabled);
    updateSettings({ caveman_mode: nextEnabled });
  };

  const handleSelectLevel = (slug: CavemanLevelSlug) => {
    onFieldChange('caveman_level', slug);
    updateSettings({ caveman_level: slug });
  };

  return (
    <div className="space-y-6">
      {/* 1. Master Caveman Enable / Disable Card */}
      <section className="rounded-2xl border border-primary/30 bg-card/70 p-5 shadow-xs">
        <div className="flex flex-wrap items-center justify-between gap-4">
          <div className="flex items-start gap-3">
            <div
              className={cn(
                'flex h-10 w-10 shrink-0 items-center justify-center rounded-xl border transition-colors',
                isEnabled
                  ? 'border-primary/40 bg-primary/15 text-primary'
                  : 'border-border/70 bg-muted/50 text-muted-foreground',
              )}
            >
              <Zap className="h-5 w-5" />
            </div>
            <div className="space-y-1">
              <div className="flex flex-wrap items-center gap-2">
                <h2 className="text-sm font-bold text-foreground">Caveman Mode (opencaveman)</h2>
                <Badge variant={isEnabled ? 'default' : 'outline'}>
                  {isEnabled ? 'Enabled' : 'Disabled'}
                </Badge>
                <Badge variant="secondary" className="font-mono text-[10.5px]">
                  {activeOption.shortBadge} · -{activeOption.targetSavingsPct}% target
                </Badge>
              </div>
              <p className="max-w-2xl text-xs leading-relaxed text-muted-foreground">
                Native Rust <span className="font-mono text-foreground">opencaveman</span> controlled-language
                engine. Cuts output and memory token usage up to 76% while keeping 100% technical
                accuracy, verbatim code blocks, and automatic safety clarity overrides.
              </p>
            </div>
          </div>

          <div className="flex items-center gap-3 rounded-xl border border-border/65 bg-background/60 px-3.5 py-2">
            <span className="text-xs font-semibold text-foreground">
              Caveman {isEnabled ? 'ON' : 'OFF'}
            </span>
            <Switch
              checked={isEnabled}
              onCheckedChange={handleToggleMode}
              aria-label="Enable Caveman Mode"
            />
          </div>
        </div>

        {/* Active Mode Live Preview Banner */}
        <div className="mt-4 rounded-xl border border-border/60 bg-background/60 p-3.5">
          <div className="flex flex-wrap items-center justify-between gap-2 text-[11px]">
            <div className="flex items-center gap-2">
              <Sparkles className="h-3.5 w-3.5 text-primary" />
              <span className="font-semibold text-foreground">
                Configured Mode: {activeOption.label}
              </span>
              <span className="rounded-md bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground">
                {SCRIPT_FAMILY_LABELS[activeOption.scriptFamily]}
              </span>
            </div>
            <span className="font-mono text-[10.5px] text-primary">
              Target compression: ~{activeOption.targetSavingsPct}% fewer output tokens
            </span>
          </div>
          <p className="mt-2 rounded-lg border border-border/50 bg-card/80 px-3 py-2 font-mono text-xs text-foreground">
            {activeOption.example}
          </p>
        </div>
      </section>

      {/* 2. Mode & Intensity Selector */}
      <section className="rounded-2xl border border-border/70 bg-card/60 p-5 shadow-xs">
        <div className="mb-4 flex flex-wrap items-center justify-between gap-3 border-b border-border/40 pb-3">
          <div className="flex items-center gap-2.5">
            <div className="flex h-8 w-8 items-center justify-center rounded-xl bg-primary/15 text-primary">
              <Languages className="h-4 w-4" />
            </div>
            <div>
              <h3 className="text-sm font-semibold text-foreground">
                Configure Caveman Intensity &amp; Script Mode
              </h3>
              <p className="text-[11px] text-muted-foreground">
                Choose the compression profile applied whenever Caveman Mode is active in Chat, TUI, or Autonomous runs.
              </p>
            </div>
          </div>

          <div className="flex flex-wrap items-center gap-1.5">
            {SCRIPT_FILTER_OPTIONS.map((filter) => {
              const selected = scriptFilter === filter.id;
              return (
                <button
                  key={filter.id}
                  type="button"
                  onClick={() => setScriptFilter(filter.id)}
                  className={cn(
                    'inline-flex items-center gap-1.5 rounded-lg border px-2.5 py-1 text-[11px] font-medium transition-colors',
                    selected
                      ? 'border-primary/50 bg-primary/12 text-primary'
                      : 'border-border/60 bg-background/50 text-muted-foreground hover:text-foreground',
                  )}
                >
                  <span>{filter.label}</span>
                  <span className="rounded bg-muted/70 px-1 font-mono text-[9.5px]">
                    {filter.badge}
                  </span>
                </button>
              );
            })}
          </div>
        </div>

        <div className="grid grid-cols-1 gap-3.5 md:grid-cols-2 lg:grid-cols-3">
          {visibleModes.map((option) => {
            const selected = activeLevel === option.slug;
            const savedForTier = stats?.byLevelSaved?.[option.slug] ?? 0;
            return (
              <button
                key={option.slug}
                type="button"
                onClick={() => handleSelectLevel(option.slug)}
                aria-pressed={selected}
                className={cn(
                  'flex flex-col justify-between rounded-xl border p-4 text-left transition-all',
                  selected
                    ? 'border-primary bg-primary/10 shadow-xs ring-1 ring-primary/25'
                    : 'border-border/70 bg-background/50 hover:border-border hover:bg-muted/30',
                )}
              >
                <div>
                  <div className="flex items-start justify-between gap-2">
                    <div>
                      <div className="flex items-center gap-1.5">
                        <span className="text-xs font-bold text-foreground">{option.label}</span>
                      </div>
                      <span className="mt-0.5 inline-block text-[10px] text-muted-foreground">
                        {SCRIPT_FAMILY_LABELS[option.scriptFamily]}
                      </span>
                    </div>
                    <span
                      className={cn(
                        'shrink-0 rounded-full px-2 py-0.5 font-mono text-[10px] font-semibold',
                        selected
                          ? 'bg-primary text-primary-foreground'
                          : 'bg-muted text-muted-foreground',
                      )}
                    >
                      -{option.targetSavingsPct}%
                    </span>
                  </div>

                  <p className="mt-2.5 text-[11px] leading-relaxed text-muted-foreground">
                    {option.summary}
                  </p>
                </div>

                <div className="mt-3 space-y-2 border-t border-border/50 pt-2.5">
                  <p className="font-mono text-[10.5px] leading-relaxed text-foreground/85">
                    {option.example}
                  </p>
                  <div className="flex items-center justify-between text-[10px]">
                    <span className="font-mono text-muted-foreground">mode: {option.slug}</span>
                    {selected ? (
                      <span className="inline-flex items-center gap-1 font-semibold text-primary">
                        <CheckCircle2 className="h-3 w-3" />
                        Selected
                      </span>
                    ) : savedForTier > 0 ? (
                      <span className="font-mono text-muted-foreground">
                        ⛏ {formatCompactTokenCount(savedForTier)} saved
                      </span>
                    ) : null}
                  </div>
                </div>
              </button>
            );
          })}
        </div>
      </section>

      {/* 3. Token-Savings Telemetry & Safety Guardrails */}
      <div className="grid grid-cols-1 gap-5 lg:grid-cols-2">
        <section className="rounded-2xl border border-border/70 bg-card/60 p-5 shadow-xs">
          <div className="mb-4 flex items-center gap-2.5 border-b border-border/40 pb-3">
            <div className="flex h-8 w-8 items-center justify-center rounded-xl bg-primary/15 text-primary">
              <Gauge className="h-4 w-4" />
            </div>
            <div>
              <h3 className="text-sm font-semibold text-foreground">
                OpenCaveman Token-Savings Telemetry
              </h3>
              <p className="text-[11px] text-muted-foreground">
                Live savings persisted in <span className="font-mono">~/.openz/caveman_stats.json</span>.
              </p>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div className="rounded-xl border border-border/60 bg-background/55 p-3">
              <div className="text-[10px] font-medium uppercase tracking-wider text-muted-foreground">
                Lifetime Total Saved
              </div>
              <div className="mt-1 font-mono text-lg font-bold text-primary">
                ⛏ {stats ? stats.compactLifetimeBadge : '0'}
              </div>
              <div className="mt-0.5 text-[10.5px] text-muted-foreground">
                Across {stats?.lifetimeTurns ?? 0} terse turns
              </div>
            </div>

            <div className="rounded-xl border border-border/60 bg-background/55 p-3">
              <div className="text-[10px] font-medium uppercase tracking-wider text-muted-foreground">
                Current Session
              </div>
              <div className="mt-1 font-mono text-lg font-bold text-foreground">
                {stats ? stats.compactSessionBadge : '0'}
              </div>
              <div className="mt-0.5 text-[10.5px] text-muted-foreground">
                {stats?.sessionTurns ?? 0} turns in active session
              </div>
            </div>

            <div className="rounded-xl border border-border/60 bg-background/55 p-3">
              <div className="text-[10px] font-medium uppercase tracking-wider text-muted-foreground">
                Output Tokens Saved
              </div>
              <div className="mt-1 font-mono text-sm font-bold text-foreground">
                {formatCompactTokenCount(stats?.lifetimeOutputTokensSaved ?? 0)}
              </div>
              <div className="mt-0.5 text-[10.5px] text-muted-foreground">
                Response generation savings
              </div>
            </div>

            <div className="rounded-xl border border-border/60 bg-background/55 p-3">
              <div className="text-[10px] font-medium uppercase tracking-wider text-muted-foreground">
                Compressed Input Files
              </div>
              <div className="mt-1 font-mono text-sm font-bold text-foreground">
                {stats?.lifetimeCompressedFiles ?? 0} files (
                {formatCompactTokenCount(stats?.lifetimeInputTokensSaved ?? 0)})
              </div>
              <div className="mt-0.5 text-[10.5px] text-muted-foreground">
                With lossless <span className="font-mono">.orig.md</span> backups
              </div>
            </div>
          </div>

          <div className="mt-4 border-t border-border/40 pt-3">
            <div className="mb-2 flex items-center gap-1.5 text-xs font-semibold text-foreground">
              <BookOpen className="h-3.5 w-3.5 text-primary" />
              <span>Built-in Procedural Skills</span>
            </div>
            <div className="grid grid-cols-1 gap-1.5 sm:grid-cols-2">
              {BUILTIN_CAVEMAN_SKILLS.map((skill) => (
                <div
                  key={skill.slug}
                  className="rounded-lg border border-border/55 bg-background/45 px-2.5 py-2"
                >
                  <div className="font-mono text-[10.5px] font-semibold text-primary">
                    {skill.slug}
                  </div>
                  <div className="mt-0.5 text-[10px] leading-snug text-muted-foreground">
                    {skill.summary}
                  </div>
                </div>
              ))}
            </div>
          </div>
        </section>

        <section className="rounded-2xl border border-border/70 bg-card/60 p-5 shadow-xs">
          <div className="mb-4 flex items-center gap-2.5 border-b border-border/40 pb-3">
            <div className="flex h-8 w-8 items-center justify-center rounded-xl bg-primary/15 text-primary">
              <ShieldCheck className="h-4 w-4" />
            </div>
            <div>
              <h3 className="text-sm font-semibold text-foreground">
                Auto-Clarity Safety Guardrails
              </h3>
              <p className="text-[11px] text-muted-foreground">
                Deterministic semantic risk scorer automatically restores full prose clarity when stakes are high.
              </p>
            </div>
          </div>

          <div className="space-y-2.5">
            {AUTO_CLARITY_GUARDRAILS.map((guardrail) => (
              <div
                key={guardrail.title}
                className="rounded-xl border border-border/60 bg-background/50 px-3.5 py-2.5"
              >
                <div className="text-xs font-semibold text-foreground">{guardrail.title}</div>
                <p className="mt-0.5 text-[11px] leading-relaxed text-muted-foreground">
                  {guardrail.detail}
                </p>
              </div>
            ))}
          </div>
        </section>
      </div>
    </div>
  );
};
