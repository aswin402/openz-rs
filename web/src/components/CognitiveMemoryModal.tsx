import React from 'react';
import { useOpenZStore } from '../store/useOpenZStore';
import { X, Brain, Key } from 'lucide-react';

export const CognitiveMemoryModal: React.FC = () => {
  const isMemoryOpen = useOpenZStore((s) => s.isMemoryOpen);
  const setIsMemoryOpen = useOpenZStore((s) => s.setIsMemoryOpen);
  const stats = useOpenZStore((s) => s.cognitiveStats);

  if (!isMemoryOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs p-4">
      <div className="w-full max-w-lg rounded-2xl border border-border bg-card p-6 shadow-2xl animate-in fade-in zoom-in-95 duration-150">
        <div className="flex items-center justify-between pb-4 border-b border-border/50">
          <div className="flex items-center gap-2 text-foreground font-semibold text-base">
            <Brain className="h-5 w-5 text-amber-500" /> Cognitive Memory & Knowledge Graph
          </div>
          <button
            onClick={() => setIsMemoryOpen(false)}
            className="rounded-lg p-1 text-muted-foreground hover:text-foreground hover:bg-muted"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <div className="mt-4 space-y-4 text-xs">
          {/* Quick Metrics */}
          <div className="grid grid-cols-3 gap-3">
            <div className="rounded-xl border border-border/60 bg-muted/30 p-3 text-center">
              <div className="text-xl font-bold text-amber-500">{stats.entitiesCount}</div>
              <div className="text-[11px] text-muted-foreground mt-0.5">Entities</div>
            </div>
            <div className="rounded-xl border border-border/60 bg-muted/30 p-3 text-center">
              <div className="text-xl font-bold text-purple-400">{stats.relationsCount}</div>
              <div className="text-[11px] text-muted-foreground mt-0.5">Relations</div>
            </div>
            <div className="rounded-xl border border-border/60 bg-muted/30 p-3 text-center">
              <div className="text-xl font-bold text-emerald-400">{stats.factsCount}</div>
              <div className="text-[11px] text-muted-foreground mt-0.5">Stored Facts</div>
            </div>
          </div>

          {/* Active Working Memory Keys */}
          <div>
            <div className="mb-2 font-semibold text-foreground flex items-center gap-1.5">
              <Key className="h-3.5 w-3.5 text-amber-500" /> Active Working Memory Scope
            </div>
            <div className="flex flex-wrap gap-1.5">
              {stats.workingMemoryKeys.map((key) => (
                <span
                  key={key}
                  className="rounded-md border border-amber-500/30 bg-amber-500/10 px-2 py-1 font-mono text-[11px] text-amber-300"
                >
                  {key}
                </span>
              ))}
            </div>
          </div>

          {/* Cognitive Database Path */}
          <div className="rounded-lg border border-border/40 bg-black/40 p-3 font-mono text-[11px] text-muted-foreground">
            <div className="font-sans font-semibold text-foreground mb-1">SQLite Database Location</div>
            ~/.openz/memory.db
          </div>
        </div>

        <div className="mt-6 flex justify-end">
          <button
            onClick={() => setIsMemoryOpen(false)}
            className="rounded-lg bg-primary px-4 py-2 text-xs font-semibold text-primary-foreground hover:opacity-90"
          >
            Done
          </button>
        </div>
      </div>
    </div>
  );
};
