import React, { useMemo, useState } from 'react';
import { useOpenZStore } from '../store/useOpenZStore';
import type { SubagentInfo } from '../types/openz';
import { ArrowLeft, Bot, Check, Copy, Cpu, Edit3, Plus, Save, Search, ShieldCheck, Trash2, X } from 'lucide-react';

const DEFAULT_PROMPT = 'You are a specialized OpenZ subagent. Focus on one clear responsibility, use available tools carefully, and return concise, actionable results.';
const DEFAULT_AGENT_NAMES = new Set([
  'orchestrator',
  'planner',
  'researcher',
  'architect',
  'skill_creator',
  'reviewer',
  'code_auditor',
  'debugger',
  'test_engineer',
  'devops_agent',
  'refactor_agent',
  'memory_manager',
  'vision_agent',
  'documentation_agent',
  'self_improvement',
  'skill_improvement',
  'openz_maintainer',
  'mcps_manager',
  'git_ops_agent',
  'ast_searcher',
  'database_specialist',
  'browser_operator',
  'dependency_manager',
  'frontend_architect',
  'docs_lookup_agent',
  'document_compiler',
  'presentation_designer',
  'code_synthesizer',
  'summarizer_agent',
  'media_designer',
  'openz_coordinator',
  'sop_designer',
  'api_integrator',
  'performance_tuner',
  'communication_manager',
  'automation_agent',
  'coding_agent',
  'diagram_designer',
  'video_animator',
]);

function normalizeAgentName(value: string): string {
  return value.trim().toLowerCase().replace(/[^a-z0-9_]+/g, '_').replace(/^_+|_+$/g, '');
}

function isProtectedAgent(name: string): boolean {
  return DEFAULT_AGENT_NAMES.has(name);
}


function fallbacksToText(fallbacks?: string[]): string {
  return (fallbacks || []).join(', ');
}

function parseFallbacks(value: string): string[] {
  return value.split(',').map((item) => item.trim()).filter(Boolean).slice(0, 3);
}

export const AgentsView: React.FC = () => {
  const subagents = useOpenZStore((s) => s.subagents);
  const setActiveView = useOpenZStore((s) => s.setActiveView);
  const saveSubagent = useOpenZStore((s) => s.saveSubagent);
  const deleteSubagent = useOpenZStore((s) => s.deleteSubagent);
  const workspaceNotice = useOpenZStore((s) => s.workspaceNotice);
  const clearWorkspaceNotice = useOpenZStore((s) => s.clearWorkspaceNotice);
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedAgentName, setSelectedAgentName] = useState<string | null>(null);
  const [draftName, setDraftName] = useState('');
  const [draftDescription, setDraftDescription] = useState('');
  const [draftPrompt, setDraftPrompt] = useState('');
  const [draftModel, setDraftModel] = useState('');
  const [draftFallbacks, setDraftFallbacks] = useState('');
  const [mode, setMode] = useState<'edit' | 'preview'>('preview');
  const [notice, setNotice] = useState<string | null>(null);

  const sortedAgents = useMemo(() => [...subagents].sort((a, b) => a.name.localeCompare(b.name)), [subagents]);
  const filteredAgents = sortedAgents.filter((agent) => {
    const query = searchQuery.toLowerCase();
    return agent.name.toLowerCase().includes(query) ||
      agent.description.toLowerCase().includes(query) ||
      agent.systemPrompt.toLowerCase().includes(query) ||
      agent.model.toLowerCase().includes(query);
  });

  const activeAgent = sortedAgents.find((agent) => agent.name === selectedAgentName);
  const normalizedName = normalizeAgentName(draftName);
  const isNew = selectedAgentName === '__new__';
  const isProtected = Boolean(activeAgent && isProtectedAgent(activeAgent.name));
  const showEditor = Boolean(selectedAgentName);
  const listPanelClass = 'w-full flex-col gap-2 overflow-y-auto pr-1 md:flex md:w-80 md:shrink-0 ' + (showEditor ? 'hidden' : 'flex');
  const editorPanelClass = 'min-w-0 flex-1 flex-col overflow-hidden rounded-2xl border border-border/80 bg-card/40 shadow-sm md:flex ' + (showEditor ? 'flex' : 'hidden');
  const nameExists = sortedAgents.some((agent) => agent.name === normalizedName && agent.name !== activeAgent?.name);
  const currentFallbacks = parseFallbacks(draftFallbacks);
  const comparableModel = draftModel.trim() || 'default';
  const isDirty = isNew
    ? Boolean(normalizedName || draftDescription.trim() || draftPrompt.trim() !== DEFAULT_PROMPT || draftModel.trim() || draftFallbacks.trim())
    : Boolean(activeAgent && (
      draftDescription !== activeAgent.description ||
      draftPrompt !== activeAgent.systemPrompt ||
      comparableModel !== activeAgent.model ||
      fallbacksToText(currentFallbacks) !== fallbacksToText(activeAgent.fallbacks)
    ));
  const canSave = Boolean(!isProtected && normalizedName && draftDescription.trim() && draftPrompt.trim() && !nameExists && isDirty);
  const pageNotice = workspaceNotice?.scope === 'agents' ? workspaceNotice : notice ? { type: 'info' as const, message: notice } : null;
  const noticeClass = pageNotice?.type === 'error'
    ? 'border-red-500/30 bg-red-500/10 text-red-300'
    : pageNotice?.type === 'success'
      ? 'border-emerald-500/30 bg-emerald-500/10 text-emerald-300'
      : 'border-amber-500/30 bg-amber-500/10 text-amber-400';

  const openAgent = (agent: SubagentInfo) => {
    setSelectedAgentName(agent.name);
    setDraftName(agent.name);
    setDraftDescription(agent.description);
    setDraftPrompt(agent.systemPrompt);
    setDraftModel(agent.model === 'default' ? '' : agent.model);
    setDraftFallbacks(fallbacksToText(agent.fallbacks));
    setMode('preview');
    setNotice(null);
    clearWorkspaceNotice('agents');
  };

  const startNewAgent = () => {
    setSelectedAgentName('__new__');
    setDraftName('');
    setDraftDescription('');
    setDraftPrompt(DEFAULT_PROMPT);
    setDraftModel('');
    setDraftFallbacks('');
    setMode('edit');
    setNotice(null);
    clearWorkspaceNotice('agents');
  };

  const duplicateAgent = () => {
    if (!activeAgent) return;
    const nextName = normalizeAgentName(activeAgent.name + '_custom');
    setSelectedAgentName('__new__');
    setDraftName(nextName);
    setDraftDescription(activeAgent.description);
    setDraftPrompt(activeAgent.systemPrompt);
    setDraftModel(activeAgent.model === 'default' ? '' : activeAgent.model);
    setDraftFallbacks(fallbacksToText(activeAgent.fallbacks));
    setMode('edit');
    setNotice('Duplicated protected profile into a new editable draft.');
  };

  const handleSave = () => {
    if (!canSave) return;
    const targetName = isNew ? normalizedName : activeAgent?.name || normalizedName;
    saveSubagent({
      name: targetName,
      description: draftDescription.trim(),
      systemPrompt: draftPrompt.trim(),
      model: draftModel.trim() || undefined,
      fallbacks: currentFallbacks,
    });
    setSelectedAgentName(targetName);
    setNotice(null);
  };

  const handleDelete = () => {
    if (!activeAgent || isProtected) return;
    deleteSubagent(activeAgent.name);
    setSelectedAgentName(null);
    setDraftName('');
    setDraftDescription('');
    setDraftPrompt('');
    setDraftModel('');
    setDraftFallbacks('');
    setNotice(null);
  };

  const handleDiscard = () => {
    if (selectedAgentName === '__new__') {
      setSelectedAgentName(null);
      return;
    }
    if (activeAgent) {
      openAgent(activeAgent);
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
          onClick={startNewAgent}
          className="flex items-center gap-1.5 rounded-lg bg-amber-500 px-3 py-1.5 text-xs font-semibold text-white shadow-sm transition hover:bg-amber-400"
        >
          <Plus className="h-3.5 w-3.5" /> New Agent
        </button>
      </div>

      <div className="flex shrink-0 flex-col justify-between gap-4 border-b border-border/50 pb-4 md:flex-row md:items-center">
        <div>
          <h1 className="flex items-center gap-2 text-2xl font-extrabold tracking-tight text-foreground">
            <Bot className="h-5 w-5 text-amber-500" /> Subagent Profiles
          </h1>
          <p className="mt-1 text-sm text-muted-foreground">Create, tune, duplicate, and delete custom OpenZ subagent profiles.</p>
        </div>

        <div className="relative w-full md:w-72">
          <Search className="absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" />
          <input
            type="text"
            placeholder="Search agents..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full rounded-lg border border-border/60 bg-card/60 py-1.5 pl-9 pr-4 text-xs text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-amber-500/50"
          />
        </div>
      </div>

      {pageNotice && (
        <div className={`mt-3 flex shrink-0 items-center justify-between rounded-lg border px-3 py-2 text-xs ${noticeClass}`}>
          <span>{pageNotice.message}</span>
          <button type="button" onClick={() => { setNotice(null); clearWorkspaceNotice('agents'); }} className="rounded p-0.5 hover:bg-amber-500/10" aria-label="Dismiss notice">
            <X className="h-3.5 w-3.5" />
          </button>
        </div>
      )}

      <div className="mt-6 flex min-h-0 flex-1 gap-6">
        <div className={listPanelClass}>
          {filteredAgents.length === 0 ? (
            <div className="rounded-2xl border border-dashed border-border/60 bg-muted/10 px-4 py-12 text-center text-xs text-muted-foreground">No subagent profiles found.</div>
          ) : (
            filteredAgents.map((agent) => {
              const selected = agent.name === selectedAgentName;
              const protectedProfile = isProtectedAgent(agent.name);
              const itemClass = 'flex w-full items-center gap-3 rounded-lg border p-3 text-left transition ' +
                (selected ? 'border-amber-500/40 bg-amber-500/10 text-amber-200' : 'border-border/60 bg-card/40 text-foreground hover:border-border hover:bg-muted/20');
              return (
                <button key={agent.name} type="button" onClick={() => openAgent(agent)} className={itemClass}>
                  <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-amber-500/20 bg-amber-500/10 text-amber-500">
                    {protectedProfile ? <ShieldCheck className="h-4 w-4" /> : <Cpu className="h-4 w-4" />}
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="truncate text-xs font-semibold leading-normal">{agent.name}</span>
                      {protectedProfile && <span className="rounded bg-muted px-1.5 py-0.5 text-[9px] font-semibold text-muted-foreground">core</span>}
                    </div>
                    <div className="mt-0.5 truncate text-[10px] text-muted-foreground">{agent.description}</div>
                    <div className="mt-1 truncate font-mono text-[9px] text-amber-400/80">{agent.model}</div>
                  </div>
                </button>
              );
            })
          )}
        </div>

        <div className={editorPanelClass}>
          {selectedAgentName ? (
            <>
              <div className="flex shrink-0 items-center justify-between gap-3 border-b border-border/60 bg-muted/30 px-4 py-3">
                <div className="flex min-w-0 items-center gap-2">
                  <button
                    type="button"
                    onClick={() => setSelectedAgentName(null)}
                    className="rounded-lg border border-border/60 bg-background/50 p-1.5 text-muted-foreground transition hover:bg-muted hover:text-foreground md:hidden"
                    aria-label="Back to agents list"
                  >
                    <ArrowLeft className="h-3.5 w-3.5" />
                  </button>
                  <Bot className="h-4 w-4 shrink-0 text-amber-500" />
                  <span className="truncate text-xs font-bold text-foreground">{isNew ? 'New Subagent' : activeAgent?.name}</span>
                  {isProtected && <span className="rounded bg-muted px-1.5 py-0.5 text-[10px] font-semibold text-muted-foreground">Protected</span>}
                  {isDirty && !isProtected && <span className="rounded bg-amber-500/10 px-1.5 py-0.5 text-[10px] font-semibold text-amber-400">Unsaved</span>}
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
                  {isProtected && activeAgent && (
                    <button
                      type="button"
                      onClick={duplicateAgent}
                      className="flex items-center gap-1.5 rounded-lg border border-border/60 bg-background/50 px-2.5 py-1.5 text-[11px] font-semibold text-muted-foreground transition hover:bg-muted hover:text-foreground"
                    >
                      <Copy className="h-3.5 w-3.5" /> Duplicate
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
                    className="flex items-center gap-1.5 rounded-lg bg-amber-500 px-2.5 py-1.5 text-[11px] font-semibold text-white transition hover:bg-amber-400 disabled:opacity-40"
                  >
                    <Save className="h-3.5 w-3.5" /> Save
                  </button>
                  {!isNew && activeAgent && !isProtected && (
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

              <div className="grid shrink-0 gap-3 border-b border-border/50 px-4 py-3 lg:grid-cols-[220px_1fr]">
                <label className="text-[11px] font-semibold text-muted-foreground">
                  Agent Name
                  <input
                    value={draftName}
                    onChange={(e) => setDraftName(e.target.value)}
                    placeholder="custom_agent"
                    readOnly={!isNew}
                    className="mt-1 w-full rounded-lg border border-border/60 bg-background px-2.5 py-2 font-mono text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500/50 read-only:cursor-not-allowed read-only:opacity-70"
                  />
                </label>
                <label className="text-[11px] font-semibold text-muted-foreground">
                  Model Override
                  <input
                    value={draftModel}
                    onChange={(e) => setDraftModel(e.target.value)}
                    placeholder="default or provider/model"
                    disabled={isProtected}
                    className="mt-1 w-full rounded-lg border border-border/60 bg-background px-2.5 py-2 font-mono text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500/50 disabled:cursor-not-allowed disabled:opacity-70"
                  />
                </label>
                <label className="text-[11px] font-semibold text-muted-foreground lg:col-span-2">
                  Description
                  <input
                    value={draftDescription}
                    onChange={(e) => setDraftDescription(e.target.value)}
                    placeholder="What this subagent is specialized in"
                    disabled={isProtected}
                    className="mt-1 w-full rounded-lg border border-border/60 bg-background px-2.5 py-2 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500/50 disabled:cursor-not-allowed disabled:opacity-70"
                  />
                </label>
                <div className="text-[11px] text-muted-foreground lg:col-span-2">
                  {isProtected ? 'Core profiles are read-only. Duplicate one to create an editable custom variant.' : nameExists ? (
                    <span className="text-red-400">A subagent named {normalizedName} already exists.</span>
                  ) : normalizedName ? (
                    <span>Saved as <code>{normalizedName}</code>. Fallbacks accept up to three comma-separated model names.</span>
                  ) : (
                    <span>Names must start with a letter and use lowercase letters, numbers, and underscores.</span>
                  )}
                </div>
              </div>

              <div className="grid min-h-0 flex-1 overflow-hidden lg:grid-cols-[1fr_260px]">
                <div className="min-h-0 overflow-hidden">
                  {mode === 'edit' && !isProtected ? (
                    <textarea
                      value={draftPrompt}
                      onChange={(e) => setDraftPrompt(e.target.value)}
                      spellCheck={false}
                      className="h-full w-full resize-none bg-background/70 p-4 font-mono text-xs leading-relaxed text-foreground outline-none"
                    />
                  ) : (
                    <div className="h-full overflow-y-auto whitespace-pre-wrap p-5 font-mono text-xs leading-relaxed text-foreground select-text">
                      {draftPrompt}
                    </div>
                  )}
                </div>
                <div className="border-t border-border/60 bg-muted/20 p-4 lg:border-l lg:border-t-0">
                  <label className="text-[11px] font-semibold text-muted-foreground">
                    Fallback Models
                    <textarea
                      value={draftFallbacks}
                      onChange={(e) => setDraftFallbacks(e.target.value)}
                      placeholder="model-a, model-b"
                      disabled={isProtected}
                      className="mt-1 h-24 w-full resize-none rounded-lg border border-border/60 bg-background px-2.5 py-2 font-mono text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500/50 disabled:cursor-not-allowed disabled:opacity-70"
                    />
                  </label>
                  <div className="mt-4 space-y-2 rounded-lg border border-border/60 bg-background/50 p-3 text-[11px] text-muted-foreground">
                    <div className="flex items-center justify-between gap-3"><span>Status</span><span>{isProtected ? 'core' : 'custom'}</span></div>
                    <div className="flex items-center justify-between gap-3"><span>Provider</span><span>{activeAgent?.provider || 'auto'}</span></div>
                    <div className="flex items-center justify-between gap-3"><span>Fallbacks</span><span>{currentFallbacks.length}</span></div>
                  </div>
                </div>
              </div>
            </>
          ) : (
            <div className="flex h-full flex-col items-center justify-center p-6 text-center">
              <Bot className="mb-3 h-10 w-10 text-muted-foreground/40" />
              <h2 className="text-sm font-bold text-foreground">Select or Create a Subagent</h2>
              <p className="mt-1 max-w-xs text-xs text-muted-foreground">Pick a profile on the left, duplicate a core profile, or create a new custom one.</p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
