import React, { useEffect, useMemo, useState } from 'react';
import { useOpenZStore } from '../../store/useOpenZStore';
import { resolveWorkspacePageNotice, workspaceNoticeBannerClass } from '../../store/chatUtils';
import { wsService } from '../../services/websocket';
import type { SkillInfo } from '../../types/openz';
import {
  ArrowLeft,
  BookOpen,
  Check,
  Edit3,
  FileText,
  History,
  Plus,
  RefreshCw,
  Save,
  Search,
  Trash2,
  X,
} from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

import { normalizeIdentifier, stripMarkdownFormatting } from '../../shared/lib/format';

const EMPTY_CONTENT =
  '---\nname: new_skill\ndescription: One-sentence summary of when the agent should load this skill\ntriggers:\n  - "example trigger"\n---\n\n# New Skill\n\nDescribe the concrete step-by-step procedure the agent should follow.\n';
const SUMMARY_LENGTH = 120;

function normalizeSkillName(value: string): string {
  return normalizeIdentifier(value, true);
}

function skillCompositeKey(skill: Pick<SkillInfo, 'name' | 'scope'>): string {
  return `${skill.scope || 'global'}:${skill.name}`;
}

function stripFrontmatterBlock(content: string): string {
  const trimmed = content.trimStart();
  const hasFence = trimmed.startsWith('---');
  const afterOpen = hasFence ? trimmed.slice(3) : '';
  const closeIdx = hasFence ? afterOpen.indexOf('---') : -1;
  return closeIdx >= 0 ? afterOpen.slice(closeIdx + 3).trimStart() : content;
}

function skillSummary(skill: SkillInfo): string {
  const explicitDesc = (skill.description || '').trim();
  const rawCandidate = explicitDesc
    ? explicitDesc
    : stripFrontmatterBlock(skill.content)
        .split('\n')
        .map((line) => stripMarkdownFormatting(line.endsWith('\r') ? line.slice(0, -1) : line))
        .find((line) => Boolean(line) && !line.startsWith('#')) || '';
  return !rawCandidate
    ? 'No description provided.'
    : rawCandidate.length > SUMMARY_LENGTH
      ? `${rawCandidate.slice(0, SUMMARY_LENGTH - 1)}…`
      : rawCandidate;
}

export const SkillsView: React.FC = () => {
  const skills = useOpenZStore((s) => s.skills);
  const setActiveView = useOpenZStore((s) => s.setActiveView);
  const saveSkill = useOpenZStore((s) => s.saveSkill);
  const deleteSkill = useOpenZStore((s) => s.deleteSkill);
  const rollbackSkill = useOpenZStore((s) => s.rollbackSkill);
  const workspaceNotice = useOpenZStore((s) => s.workspaceNotice);
  const clearWorkspaceNotice = useOpenZStore((s) => s.clearWorkspaceNotice);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [draftName, setDraftName] = useState('');
  const [draftContent, setDraftContent] = useState('');
  const [mode, setMode] = useState<'edit' | 'preview'>('edit');
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    wsService.requestConfig();
  }, []);

  const sortedSkills = useMemo(
    () =>
      [...skills].sort(
        (a, b) =>
          (a.scope || 'global').localeCompare(b.scope || 'global') ||
          a.name.localeCompare(b.name),
      ),
    [skills],
  );
  const needle = searchQuery.toLowerCase();
  const filteredSkills = sortedSkills.filter(
    (skill) =>
      skill.name.toLowerCase().includes(needle) ||
      (skill.description || '').toLowerCase().includes(needle) ||
      (skill.triggers || []).some((t) => t.toLowerCase().includes(needle)) ||
      skill.content.toLowerCase().includes(needle),
  );

  const activeSkill = sortedSkills.find((skill) => skillCompositeKey(skill) === selectedKey);
  const activeValidationErrors = activeSkill?.validationErrors || [];
  const isProtected = Boolean(activeSkill?.isProtected);
  const normalizedName = normalizeSkillName(draftName);
  const isNew = selectedKey === '__new__';
  const showEditor = Boolean(selectedKey);
  const listPanelClass =
    'w-full flex-col gap-2 overflow-y-auto pr-1 md:flex md:w-80 md:shrink-0 ' +
    (showEditor ? 'hidden' : 'flex');
  const editorPanelClass =
    'min-w-0 flex-1 flex-col overflow-hidden rounded-2xl border border-border/80 bg-card/40 shadow-sm md:flex ' +
    (showEditor ? 'flex' : 'hidden');
  const isDirty = isNew
    ? Boolean(normalizedName || draftContent.trim() !== EMPTY_CONTENT.trim())
    : Boolean(activeSkill && draftContent !== activeSkill.content);
  const nameExists = sortedSkills.some(
    (skill) =>
      (skill.scope || 'global') === 'global' &&
      skill.name === normalizedName &&
      skillCompositeKey(skill) !== selectedKey,
  );
  const canSave = Boolean(
    !isProtected && normalizedName && draftContent.trim() && !nameExists && isDirty,
  );
  const canRollback = Boolean(
    !isNew && activeSkill && !isProtected && (activeSkill.revisionCount || 0) > 0,
  );
  const skillStats = useMemo(
    () => ({
      total: skills.length,
      enabled: skills.filter((skill) => skill.enabled !== false).length,
      protected: skills.filter((skill) => skill.isProtected).length,
      invalid: skills.filter((skill) => (skill.validationErrors?.length || 0) > 0).length,
    }),
    [skills],
  );
  const pageNotice = resolveWorkspacePageNotice(workspaceNotice, 'skills', notice);
  const noticeClass = workspaceNoticeBannerClass(pageNotice?.type);

  const openSkill = (skill: SkillInfo) => {
    setSelectedKey(skillCompositeKey(skill));
    setDraftName(skill.name);
    setDraftContent(skill.content);
    setMode('preview');
    setNotice(null);
    clearWorkspaceNotice('skills');
  };

  const startNewSkill = () => {
    setSelectedKey('__new__');
    setDraftName('');
    setDraftContent(EMPTY_CONTENT);
    setMode('edit');
    setNotice(null);
    clearWorkspaceNotice('skills');
  };

  const handleSave = () => {
    canSave &&
      (() => {
        const targetName = isNew ? normalizedName : activeSkill?.name || normalizedName;
        const targetProfile = isNew ? null : activeSkill?.profile || null;
        const nextScope = targetProfile ? `subagent:${targetProfile}` : 'global';
        saveSkill(targetName, draftContent, targetProfile);
        setSelectedKey(`${nextScope}:${targetName}`);
        setNotice(null);
      })();
  };

  const handleDelete = () => {
    activeSkill &&
      !isProtected &&
      (() => {
        deleteSkill(activeSkill.name, activeSkill.profile || null);
        setSelectedKey(null);
        setDraftName('');
        setDraftContent('');
        setNotice(null);
      })();
  };

  const handleRollback = () => {
    activeSkill &&
      canRollback &&
      (() => {
        rollbackSkill(activeSkill.name, activeSkill.profile || null);
        setNotice(null);
      })();
  };

  const handleDiscard = () => {
    selectedKey === '__new__'
      ? (() => {
          setSelectedKey(null);
          setDraftName('');
          setDraftContent('');
        })()
      : activeSkill &&
        (() => {
          setDraftName(activeSkill.name);
          setDraftContent(activeSkill.content);
          setMode('preview');
        })();
  };

  return (
    <div className="mx-auto flex h-full max-w-6xl flex-col overflow-hidden px-4 py-6">
      <div className="mb-4 flex shrink-0 items-center justify-between gap-3">
        <button
          onClick={() => setActiveView('dashboard')}
          className="flex items-center gap-1.5 rounded-lg border border-border/60 bg-muted/20 px-3 py-1.5 text-xs font-semibold text-muted-foreground transition-colors hover:bg-muted/40 hover:text-foreground"
        >
          <ArrowLeft className="h-3.5 w-3.5" /> Go Back
        </button>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => wsService.requestConfig()}
            className="flex items-center gap-1.5 rounded-lg border border-border/60 bg-muted/20 px-3 py-1.5 text-xs font-semibold text-muted-foreground transition hover:bg-muted/40 hover:text-foreground"
          >
            <RefreshCw className="h-3.5 w-3.5" /> Refresh
          </button>
          <button
            type="button"
            onClick={startNewSkill}
            className="flex items-center gap-1.5 rounded-lg bg-primary px-3 py-1.5 text-xs font-semibold text-primary-foreground shadow-sm transition hover:opacity-90"
          >
            <Plus className="h-3.5 w-3.5" /> New Skill
          </button>
        </div>
      </div>

      <div className="mt-4 grid shrink-0 grid-cols-2 gap-2 sm:grid-cols-4">
        <SkillMetric label="Total" value={skillStats.total} />
        <SkillMetric label="Enabled" value={skillStats.enabled} />
        <SkillMetric label="Read-only" value={skillStats.protected} />
        <SkillMetric label="Needs review" value={skillStats.invalid} />
      </div>

      <div className="flex shrink-0 flex-col justify-between gap-4 border-b border-border/50 pb-4 md:flex-row md:items-center">
        <div>
          <h1 className="flex items-center gap-2 text-2xl font-extrabold tracking-tight text-foreground">
            <BookOpen className="h-5 w-5 text-primary" /> AI Agent Skills
          </h1>
          <p className="mt-1 text-sm text-muted-foreground">
            Progressive-disclosure procedural skills with BM25F routing, usage telemetry, and 10-snapshot rollback.
          </p>
        </div>

        <div className="relative w-full md:w-72">
          <Search className="absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" />
          <input
            type="text"
            placeholder="Search skills, triggers, descriptions..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full rounded-lg border border-border/60 bg-card/60 py-1.5 pl-9 pr-4 text-xs text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-primary/50"
          />
        </div>
      </div>

      {pageNotice && (
        <div
          className={`mt-3 flex shrink-0 items-center justify-between rounded-lg border px-3 py-2 text-xs ${noticeClass}`}
        >
          <span>{pageNotice.message}</span>
          <button
            type="button"
            onClick={() => {
              setNotice(null);
              clearWorkspaceNotice('skills');
            }}
            className="rounded p-0.5 hover:bg-primary/10"
            aria-label="Dismiss notice"
          >
            <X className="h-3.5 w-3.5" />
          </button>
        </div>
      )}

      <div className="mt-6 flex min-h-0 flex-1 gap-6">
        <div className={listPanelClass}>
          {filteredSkills.length === 0 ? (
            <div className="rounded-2xl border border-dashed border-border/60 bg-muted/10 px-4 py-12 text-center text-xs text-muted-foreground">
              No skills found.
            </div>
          ) : (
            filteredSkills.map((skill) => {
              const compKey = skillCompositeKey(skill);
              const isSelected = compKey === selectedKey;
              const itemClass =
                'flex w-full items-center gap-3 rounded-lg border p-3 text-left transition ' +
                (isSelected
                  ? 'border-primary/40 bg-primary/10 text-primary'
                  : 'border-border/60 bg-card/40 text-foreground hover:border-border hover:bg-muted/20');
              const iconBoxClass =
                'flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border ' +
                (isSelected ? 'border-primary/20 bg-primary/15' : 'border-border/80 bg-muted/40');
              const iconClass = 'h-4 w-4 ' + (isSelected ? 'text-primary' : 'text-muted-foreground');
              return (
                <button key={compKey} onClick={() => openSkill(skill)} className={itemClass}>
                  <div className={iconBoxClass}>
                    <FileText className={iconClass} />
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <div className="truncate text-xs font-semibold leading-normal">{skill.name}</div>
                      {skill.isProtected ? (
                        <span className="rounded bg-muted px-1.5 py-0.5 text-[9px] font-semibold text-muted-foreground">
                          read-only
                        </span>
                      ) : null}
                      {skill.validationErrors?.length ? (
                        <span className="rounded bg-destructive/12 px-1.5 py-0.5 text-[9px] font-semibold text-destructive">
                          invalid
                        </span>
                      ) : null}
                    </div>
                    <div className="mt-0.5 truncate text-[10px] text-muted-foreground">
                      {skillSummary(skill)}
                    </div>
                    <div className="mt-1 flex flex-wrap items-center gap-1.5 text-[9px] font-medium text-muted-foreground/80">
                      <span className="rounded bg-muted px-1.5 py-0.5">{skill.scope || 'global'}</span>
                      <span className="rounded bg-muted px-1.5 py-0.5">{skill.source || 'unknown'}</span>
                      {(skill.useCount || 0) > 0 ? (
                        <span className="rounded bg-muted px-1.5 py-0.5">
                          {skill.useCount} uses
                        </span>
                      ) : null}
                      {(skill.revisionCount || 0) > 0 ? (
                        <span className="rounded bg-muted px-1.5 py-0.5">
                          {skill.revisionCount} rev
                        </span>
                      ) : null}
                    </div>
                  </div>
                </button>
              );
            })
          )}
        </div>

        <div className={editorPanelClass}>
          {selectedKey ? (
            <>
              <div className="flex shrink-0 items-center justify-between gap-3 border-b border-border/60 bg-muted/30 px-4 py-3">
                <div className="flex min-w-0 items-center gap-2">
                  <button
                    type="button"
                    onClick={() => setSelectedKey(null)}
                    className="rounded-lg border border-border/60 bg-background/50 p-1.5 text-muted-foreground transition hover:bg-muted hover:text-foreground md:hidden"
                    aria-label="Back to skills list"
                  >
                    <ArrowLeft className="h-3.5 w-3.5" />
                  </button>
                  <FileText className="h-4 w-4 shrink-0 text-primary" />
                  <span className="truncate text-xs font-bold text-foreground">
                    {isNew ? 'New Skill' : activeSkill?.name}
                  </span>
                  {!isNew && activeSkill?.scope && (
                    <span className="rounded bg-muted px-1.5 py-0.5 text-[10px] font-semibold text-muted-foreground">
                      {activeSkill.scope}
                    </span>
                  )}
                  {isProtected && (
                    <span className="rounded bg-muted px-1.5 py-0.5 text-[10px] font-semibold text-muted-foreground">
                      Read-only
                    </span>
                  )}
                  {activeValidationErrors.length > 0 && (
                    <span className="rounded bg-destructive/12 px-1.5 py-0.5 text-[10px] font-semibold text-destructive">
                      Invalid
                    </span>
                  )}
                  {isDirty && (
                    <span className="rounded bg-warning/12 px-1.5 py-0.5 text-[10px] font-semibold text-warning">
                      Unsaved
                    </span>
                  )}
                </div>
                <div className="flex shrink-0 flex-wrap items-center justify-end gap-2">
                  <button
                    type="button"
                    onClick={() => setMode(mode === 'edit' ? 'preview' : 'edit')}
                    disabled={isProtected}
                    className="flex items-center gap-1.5 rounded-lg border border-border/60 bg-background/50 px-2.5 py-1.5 text-[11px] font-semibold text-muted-foreground transition hover:bg-muted hover:text-foreground disabled:cursor-not-allowed disabled:opacity-40"
                  >
                    {mode === 'edit' ? (
                      <Check className="h-3.5 w-3.5" />
                    ) : (
                      <Edit3 className="h-3.5 w-3.5" />
                    )}
                    {mode === 'edit' ? 'Preview' : 'Edit'}
                  </button>
                  {canRollback && (
                    <button
                      type="button"
                      onClick={handleRollback}
                      className="flex items-center gap-1.5 rounded-lg border border-border/60 bg-background/50 px-2.5 py-1.5 text-[11px] font-semibold text-muted-foreground transition hover:bg-muted hover:text-foreground"
                      title="Restore previous snapshot from SQLite revision history"
                    >
                      <History className="h-3.5 w-3.5" /> Rollback ({activeSkill?.revisionCount})
                    </button>
                  )}
                  <button
                    type="button"
                    onClick={handleDiscard}
                    disabled={!isDirty}
                    className="rounded-lg border border-border/60 bg-background/50 px-2.5 py-1.5 text-[11px] font-semibold text-muted-foreground transition hover:bg-muted hover:text-foreground disabled:opacity-40"
                  >
                    Discard
                  </button>
                  <button
                    type="button"
                    onClick={handleSave}
                    disabled={!canSave}
                    className="flex items-center gap-1.5 rounded-lg bg-primary px-2.5 py-1.5 text-[11px] font-semibold text-primary-foreground transition hover:opacity-90 disabled:opacity-40"
                  >
                    <Save className="h-3.5 w-3.5" /> Save
                  </button>
                  {!isNew && activeSkill && !isProtected && (
                    <button
                      type="button"
                      onClick={handleDelete}
                      className="flex items-center gap-1.5 rounded-lg border border-destructive/30 bg-destructive/10 px-2.5 py-1.5 text-[11px] font-semibold text-destructive transition hover:bg-destructive/20"
                    >
                      <Trash2 className="h-3.5 w-3.5" /> Delete
                    </button>
                  )}
                </div>
              </div>

              <div className="grid shrink-0 gap-2 border-b border-border/50 px-4 py-3 md:grid-cols-[220px_1fr]">
                <label className="text-[11px] font-semibold text-muted-foreground">
                  Skill Name
                  <input
                    value={draftName}
                    onChange={(e) => setDraftName(e.target.value)}
                    placeholder="skill_name"
                    readOnly={!isNew}
                    className="mt-1 w-full rounded-lg border border-border/60 bg-background px-2.5 py-2 font-mono text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-primary/50 read-only:cursor-not-allowed read-only:opacity-70"
                  />
                </label>
                <div className="flex flex-col justify-end gap-1 text-[11px] text-muted-foreground">
                  {nameExists ? (
                    <span className="text-destructive">
                      A skill named {normalizedName} already exists.
                    </span>
                  ) : normalizedName ? (
                    <span>
                      Saved as <code>{normalizedName}</code>
                    </span>
                  ) : (
                    <span>
                      {isNew
                        ? 'Use lowercase letters, numbers, dash, or underscore.'
                        : 'Existing skill names are fixed. Create a new skill to use another name.'}
                    </span>
                  )}
                  {!isNew && activeSkill && (
                    <span className="truncate">
                      Source: {activeSkill.source || 'unknown'}
                      {activeSkill.path ? ` · ${activeSkill.path}` : ''}
                      {(activeSkill.triggers?.length || 0) > 0
                        ? ` · Triggers: ${activeSkill.triggers?.slice(0, 4).join(', ')}`
                        : ''}
                    </span>
                  )}
                </div>
              </div>

              {activeValidationErrors.length > 0 && (
                <div className="border-b border-destructive/25 bg-destructive/10 px-4 py-2 text-[11px] text-destructive">
                  {activeValidationErrors.join(' ')}
                </div>
              )}

              <div className="min-h-0 flex-1 overflow-hidden">
                {mode === 'edit' && !isProtected ? (
                  <textarea
                    value={draftContent}
                    onChange={(e) => setDraftContent(e.target.value)}
                    spellCheck={false}
                    className="h-full w-full resize-none bg-background/70 p-4 font-mono text-xs leading-relaxed text-foreground outline-none"
                  />
                ) : (
                  <div className="h-full overflow-y-auto p-5 prose dark:prose-invert prose-xs max-w-none break-words select-text">
                    <ReactMarkdown remarkPlugins={[remarkGfm]}>{draftContent}</ReactMarkdown>
                  </div>
                )}
              </div>
            </>
          ) : (
            <div className="flex h-full flex-col items-center justify-center p-6 text-center">
              <BookOpen className="mb-3 h-10 w-10 text-muted-foreground/40" />
              <h2 className="text-sm font-bold text-foreground">Select or Create a Skill</h2>
              <p className="mt-1 max-w-xs text-xs text-muted-foreground">
                Pick a skill on the left or create a new one to edit its markdown instructions.
              </p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

const SkillMetric: React.FC<{ label: string; value: number }> = ({ label, value }) => (
  <div className="rounded-xl border border-border/60 bg-card/45 px-3 py-2 shadow-sm">
    <div className="text-[10px] font-bold uppercase tracking-wider text-muted-foreground/70">
      {label}
    </div>
    <div className="mt-0.5 text-lg font-extrabold text-foreground">{value}</div>
  </div>
);
