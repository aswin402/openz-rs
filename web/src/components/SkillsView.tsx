import React, { useMemo, useState } from 'react';
import { useOpenZStore } from '../store/useOpenZStore';
import { ArrowLeft, BookOpen, Check, Edit3, FileText, Plus, Save, Search, Trash2, X } from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

const EMPTY_CONTENT = '# New Skill\n\nDescribe when the agent should use this skill and the concrete procedure it should follow.\n';

function normalizeSkillName(value: string): string {
  return value.trim().toLowerCase().replace(/[^a-z0-9_-]+/g, '_').replace(/^_+|_+$/g, '');
}

function skillSummary(content: string): string {
  return content.slice(0, 72).replace(/[#*_-]/g, '');
}

export const SkillsView: React.FC = () => {
  const skills = useOpenZStore((s) => s.skills);
  const setActiveView = useOpenZStore((s) => s.setActiveView);
  const saveSkill = useOpenZStore((s) => s.saveSkill);
  const deleteSkill = useOpenZStore((s) => s.deleteSkill);
  const workspaceNotice = useOpenZStore((s) => s.workspaceNotice);
  const clearWorkspaceNotice = useOpenZStore((s) => s.clearWorkspaceNotice);
  const [selectedSkill, setSelectedSkill] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [draftName, setDraftName] = useState('');
  const [draftContent, setDraftContent] = useState('');
  const [mode, setMode] = useState<'edit' | 'preview'>('edit');
  const [notice, setNotice] = useState<string | null>(null);

  const sortedSkills = useMemo(() => [...skills].sort((a, b) => a.name.localeCompare(b.name)), [skills]);
  const filteredSkills = sortedSkills.filter((skill) =>
    skill.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
    skill.content.toLowerCase().includes(searchQuery.toLowerCase())
  );

  const activeSkill = sortedSkills.find((skill) => skill.name === selectedSkill);
  const normalizedName = normalizeSkillName(draftName);
  const isNew = selectedSkill === '__new__';
  const showEditor = Boolean(selectedSkill);
  const listPanelClass = 'w-full flex-col gap-2 overflow-y-auto pr-1 md:flex md:w-72 md:shrink-0 ' +
    (showEditor ? 'hidden' : 'flex');
  const editorPanelClass = 'min-w-0 flex-1 flex-col overflow-hidden rounded-2xl border border-border/80 bg-card/40 shadow-sm md:flex ' +
    (showEditor ? 'flex' : 'hidden');
  const isDirty = isNew
    ? Boolean(normalizedName || draftContent.trim() !== EMPTY_CONTENT.trim())
    : Boolean(activeSkill && draftContent !== activeSkill.content);
  const nameExists = sortedSkills.some((skill) => skill.name === normalizedName && skill.name !== activeSkill?.name);
  const canSave = Boolean(normalizedName && draftContent.trim() && !nameExists && isDirty);
  const pageNotice = workspaceNotice?.scope === 'skills' ? workspaceNotice : notice ? { type: 'info' as const, message: notice } : null;
  const noticeClass = pageNotice?.type === 'error'
    ? 'border-red-500/30 bg-red-500/10 text-red-300'
    : pageNotice?.type === 'success'
      ? 'border-emerald-500/30 bg-emerald-500/10 text-emerald-300'
      : 'border-amber-500/30 bg-amber-500/10 text-amber-400';


  const openSkill = (skill: { name: string; content: string }) => {
    setSelectedSkill(skill.name);
    setDraftName(skill.name);
    setDraftContent(skill.content);
    setMode('preview');
    setNotice(null);
    clearWorkspaceNotice('skills');
  };

  const startNewSkill = () => {
    setSelectedSkill('__new__');
    setDraftName('');
    setDraftContent(EMPTY_CONTENT);
    setMode('edit');
    setNotice(null);
    clearWorkspaceNotice('skills');
  };

  const handleSave = () => {
    if (!canSave) return;
    const targetName = isNew ? normalizedName : activeSkill?.name || normalizedName;
    saveSkill(targetName, draftContent);
    setSelectedSkill(targetName);
    setNotice(null);
  };

  const handleDelete = () => {
    if (!activeSkill) return;
    deleteSkill(activeSkill.name);
    setSelectedSkill(null);
    setDraftName('');
    setDraftContent('');
    setNotice(null);
  };

  const handleDiscard = () => {
    if (selectedSkill === '__new__') {
      setSelectedSkill(null);
      setDraftName('');
      setDraftContent('');
      return;
    }
    if (activeSkill) {
      setDraftName(activeSkill.name);
      setDraftContent(activeSkill.content);
      setMode('preview');
    }
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
        <button
          type="button"
          onClick={startNewSkill}
          className="flex items-center gap-1.5 rounded-lg bg-amber-500 px-3 py-1.5 text-xs font-semibold text-white shadow-sm transition hover:bg-amber-400"
        >
          <Plus className="h-3.5 w-3.5" /> New Skill
        </button>
      </div>

      <div className="flex shrink-0 flex-col justify-between gap-4 border-b border-border/50 pb-4 md:flex-row md:items-center">
        <div>
          <h1 className="flex items-center gap-2 text-2xl font-extrabold tracking-tight text-foreground">
            <BookOpen className="h-5 w-5 text-amber-500" /> AI Agent Skills
          </h1>
          <p className="mt-1 text-sm text-muted-foreground">Browse, edit, create, and delete runtime skills stored by OpenZ.</p>
        </div>

        <div className="relative w-full md:w-72">
          <Search className="absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" />
          <input
            type="text"
            placeholder="Search skills..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full rounded-lg border border-border/60 bg-card/60 py-1.5 pl-9 pr-4 text-xs text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-amber-500/50"
          />
        </div>
      </div>

      {pageNotice && (
        <div className={`mt-3 flex shrink-0 items-center justify-between rounded-lg border px-3 py-2 text-xs ${noticeClass}`}>
          <span>{pageNotice.message}</span>
          <button type="button" onClick={() => { setNotice(null); clearWorkspaceNotice('skills'); }} className="rounded p-0.5 hover:bg-amber-500/10" aria-label="Dismiss notice">
            <X className="h-3.5 w-3.5" />
          </button>
        </div>
      )}

      <div className="mt-6 flex min-h-0 flex-1 gap-6">
        <div className={listPanelClass}>
          {filteredSkills.length === 0 ? (
            <div className="rounded-2xl border border-dashed border-border/60 bg-muted/10 px-4 py-12 text-center text-xs text-muted-foreground">No skills found.</div>
          ) : (
            filteredSkills.map((skill) => {
              const isSelected = skill.name === selectedSkill;
              const itemClass = 'flex w-full items-center gap-3 rounded-lg border p-3 text-left transition ' +
                (isSelected
                  ? 'border-amber-500/40 bg-amber-500/10 text-amber-200'
                  : 'border-border/60 bg-card/40 text-foreground hover:border-border hover:bg-muted/20');
              const iconBoxClass = 'flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border ' +
                (isSelected ? 'border-amber-500/20 bg-amber-500/20' : 'border-border/80 bg-muted/40');
              const iconClass = 'h-4 w-4 ' + (isSelected ? 'text-amber-400' : 'text-muted-foreground');
              return (
                <button key={skill.name} onClick={() => openSkill(skill)} className={itemClass}>
                  <div className={iconBoxClass}>
                    <FileText className={iconClass} />
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-xs font-semibold leading-normal">{skill.name}</div>
                    <div className="mt-0.5 truncate text-[10px] text-muted-foreground">{skillSummary(skill.content)}</div>
                  </div>
                </button>
              );
            })
          )}
        </div>

        <div className={editorPanelClass}>
          {selectedSkill ? (
            <>
              <div className="flex shrink-0 items-center justify-between gap-3 border-b border-border/60 bg-muted/30 px-4 py-3">
                <div className="flex min-w-0 items-center gap-2">
                  <button
                    type="button"
                    onClick={() => setSelectedSkill(null)}
                    className="rounded-lg border border-border/60 bg-background/50 p-1.5 text-muted-foreground transition hover:bg-muted hover:text-foreground md:hidden"
                    aria-label="Back to skills list"
                  >
                    <ArrowLeft className="h-3.5 w-3.5" />
                  </button>
                  <FileText className="h-4 w-4 shrink-0 text-amber-500" />
                  <span className="truncate text-xs font-bold text-foreground">{isNew ? 'New Skill' : activeSkill?.name}</span>
                  {isDirty && <span className="rounded bg-amber-500/10 px-1.5 py-0.5 text-[10px] font-semibold text-amber-400">Unsaved</span>}
                </div>
                <div className="flex shrink-0 flex-wrap items-center justify-end gap-2">
                  <button
                    type="button"
                    onClick={() => setMode(mode === 'edit' ? 'preview' : 'edit')}
                    className="flex items-center gap-1.5 rounded-lg border border-border/60 bg-background/50 px-2.5 py-1.5 text-[11px] font-semibold text-muted-foreground transition hover:bg-muted hover:text-foreground"
                  >
                    {mode === 'edit' ? <Check className="h-3.5 w-3.5" /> : <Edit3 className="h-3.5 w-3.5" />}
                    {mode === 'edit' ? 'Preview' : 'Edit'}
                  </button>
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
                    className="flex items-center gap-1.5 rounded-lg bg-amber-500 px-2.5 py-1.5 text-[11px] font-semibold text-white transition hover:bg-amber-400 disabled:opacity-40"
                  >
                    <Save className="h-3.5 w-3.5" /> Save
                  </button>
                  {!isNew && activeSkill && (
                    <button
                      type="button"
                      onClick={handleDelete}
                      className="flex items-center gap-1.5 rounded-lg border border-red-500/30 bg-red-500/10 px-2.5 py-1.5 text-[11px] font-semibold text-red-300 transition hover:bg-red-500/20"
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
                    className="mt-1 w-full rounded-lg border border-border/60 bg-background px-2.5 py-2 font-mono text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500/50 read-only:cursor-not-allowed read-only:opacity-70"
                  />
                </label>
                <div className="flex items-end text-[11px] text-muted-foreground">
                  {nameExists ? (
                    <span className="text-red-400">A skill named {normalizedName} already exists.</span>
                  ) : normalizedName ? (
                    <span>Saved as <code>{normalizedName}</code></span>
                  ) : (
                    <span>{isNew ? 'Use lowercase letters, numbers, dash, or underscore.' : 'Existing skill names are fixed. Create a new skill to use another name.'}</span>
                  )}
                </div>
              </div>

              <div className="min-h-0 flex-1 overflow-hidden">
                {mode === 'edit' ? (
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
              <p className="mt-1 max-w-xs text-xs text-muted-foreground">Pick a skill on the left or create a new one to edit its markdown instructions.</p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
