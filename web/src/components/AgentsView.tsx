import React, { useState } from 'react';
import { useOpenZStore } from '../store/useOpenZStore';
import { ArrowLeft, Bot, Search, Cpu, Terminal, X } from 'lucide-react';

export const AgentsView: React.FC = () => {
  const subagents = useOpenZStore((s) => s.subagents);
  const setActiveView = useOpenZStore((s) => s.setActiveView);
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedAgentName, setSelectedAgentName] = useState<string | null>(null);

  const filteredAgents = subagents.filter((agent) =>
    agent.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
    agent.description.toLowerCase().includes(searchQuery.toLowerCase()) ||
    agent.systemPrompt.toLowerCase().includes(searchQuery.toLowerCase())
  );

  const activeAgent = subagents.find((a) => a.name === selectedAgentName);

  return (
    <div className="mx-auto max-w-5xl px-4 py-6 h-full flex flex-col overflow-hidden">
      {/* Back button */}
      <div className="mb-4 shrink-0">
        <button
          onClick={() => setActiveView('dashboard')}
          className="flex items-center gap-1.5 rounded-lg border border-border/60 bg-muted/20 px-3 py-1.5 text-xs font-semibold text-muted-foreground hover:text-foreground hover:bg-muted/40 transition-colors"
        >
          <ArrowLeft className="h-3.5 w-3.5" /> Go Back
        </button>
      </div>

      {/* Header section */}
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-4 pb-4 border-b border-border/50 shrink-0">
        <div>
          <h1 className="flex items-center gap-2 text-2xl font-extrabold tracking-tight text-foreground">
            <Bot className="h-5 w-5 text-amber-500" /> Subagent Profiles
          </h1>
          <p className="mt-1 text-sm text-muted-foreground">
            Orchestrated subagent profiles configured in <code>~/.openz/subagents.json</code>
          </p>
        </div>

        <div className="relative w-full md:w-64">
          <Search className="absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" />
          <input
            type="text"
            placeholder="Search subagents..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full rounded-lg border border-border/60 bg-card/60 pl-9 pr-4 py-1.5 text-xs text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-amber-500/50"
          />
        </div>
      </div>

      {/* Grid wrapper */}
      <div className="mt-6 flex-1 overflow-y-auto pr-1 min-h-0">
        {filteredAgents.length === 0 ? (
          <div className="py-16 text-center text-xs text-muted-foreground border border-dashed border-border/60 rounded-2xl bg-muted/10">
            No subagent profiles found. Configure them inside the subagents JSON backend.
          </div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {filteredAgents.map((agent) => (
              <div
                key={agent.name}
                className="flex flex-col justify-between rounded-2xl border border-border/60 bg-card/40 p-4 shadow-sm hover:border-border hover:bg-muted/10 transition duration-150"
              >
                <div>
                  <div className="flex items-center gap-3">
                    <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-amber-500/10 text-amber-500 border border-amber-500/20">
                      <Cpu className="h-5 w-5" />
                    </div>
                    <div className="min-w-0">
                      <h2 className="font-extrabold text-sm text-foreground truncate">{agent.name}</h2>
                      <div className="flex flex-wrap gap-1.5 mt-1">
                        <span className="rounded bg-muted px-1.5 py-0.5 text-[9px] font-semibold text-muted-foreground">
                          {agent.provider}
                        </span>
                        <span className="rounded bg-amber-500/10 px-1.5 py-0.5 text-[9px] font-semibold text-amber-400 border border-amber-500/10 max-w-[120px] truncate" title={agent.model}>
                          {agent.model}
                        </span>
                      </div>
                    </div>
                  </div>

                  <p className="mt-3 text-xs leading-relaxed text-muted-foreground min-h-[48px] line-clamp-3">
                    {agent.description}
                  </p>
                </div>

                <div className="mt-4 pt-3 border-t border-border/40 flex justify-end">
                  <button
                    onClick={() => setSelectedAgentName(agent.name)}
                    className="flex items-center gap-1.5 rounded-lg border border-border/80 bg-muted/40 px-3 py-1.5 text-[11px] font-semibold text-foreground hover:bg-muted transition"
                  >
                    <Terminal className="h-3 w-3 text-amber-500" /> View System Prompt
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* System Prompt View Modal */}
      {selectedAgentName && activeAgent && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs p-4">
          <div className="w-full max-w-2xl rounded-2xl border border-border bg-card p-6 shadow-2xl animate-in fade-in zoom-in-95 duration-150 flex flex-col max-h-[85vh]">
            <div className="flex items-center justify-between pb-4 border-b border-border/50">
              <div className="flex items-center gap-2">
                <div className="flex h-7 w-7 items-center justify-center rounded-lg bg-amber-500/10 text-amber-500">
                  <Bot className="h-4 w-4" />
                </div>
                <div>
                  <h3 className="font-extrabold text-sm text-foreground">{activeAgent.name}</h3>
                  <p className="text-[10px] text-muted-foreground mt-0.5">{activeAgent.provider} / {activeAgent.model}</p>
                </div>
              </div>
              <button
                onClick={() => setSelectedAgentName(null)}
                className="rounded-lg p-1 text-muted-foreground hover:text-foreground hover:bg-muted"
              >
                <X className="h-4 w-4" />
              </button>
            </div>

            <div className="mt-4 flex-1 min-h-0 overflow-y-auto bg-black/40 border border-border/60 rounded-xl p-4 font-mono text-[11px] leading-relaxed text-amber-100/90 whitespace-pre-wrap select-text">
              {activeAgent.systemPrompt}
            </div>

            <div className="mt-6 flex justify-end">
              <button
                onClick={() => setSelectedAgentName(null)}
                className="rounded-lg bg-primary px-4 py-2 text-xs font-semibold text-primary-foreground hover:opacity-90"
              >
                Close
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
