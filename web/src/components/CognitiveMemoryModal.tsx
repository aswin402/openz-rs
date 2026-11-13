import React from 'react';
import { useOpenZStore } from '../store/useOpenZStore';
import { X, Brain, Database, GitBranch, Key } from 'lucide-react';
import { MetricCard } from '../shared/ui/MetricCard';
import { Badge } from './ui/badge';
import { Button } from './ui/button';

export const CognitiveMemoryModal: React.FC = () => {
  const isMemoryOpen = useOpenZStore((s) => s.isMemoryOpen);
  const setIsMemoryOpen = useOpenZStore((s) => s.setIsMemoryOpen);
  const setActiveView = useOpenZStore((s) => s.setActiveView);
  const stats = useOpenZStore((s) => s.cognitiveStats);

  if (!isMemoryOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-foreground/45 p-4 backdrop-blur-xs">
      <div className="w-full max-w-lg rounded-2xl border border-border bg-card p-6 shadow-2xl">
        <div className="flex items-center justify-between border-b border-border/50 pb-4">
          <div className="flex items-center gap-2 text-base font-semibold text-foreground">
            <Brain className="size-5 text-primary" />
            <span>Cognitive Memory &amp; Knowledge Graph</span>
          </div>
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            onClick={() => setIsMemoryOpen(false)}
            aria-label="Close memory modal"
          >
            <X className="size-4" />
          </Button>
        </div>

        <div className="mt-4 space-y-4 text-xs">
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
            <MetricCard label="Entities" value={String(stats.entitiesCount)} icon={Brain} />
            <MetricCard
              label="Relations"
              value={String(stats.relationsCount)}
              icon={GitBranch}
              accent="text-primary"
            />
            <MetricCard
              label="Stored Facts"
              value={String(stats.factsCount)}
              icon={Database}
              accent="text-success"
            />
          </div>

          <div>
            <div className="mb-2 flex items-center gap-1.5 font-semibold text-foreground">
              <Key className="size-3.5 text-primary" />
              <span>Active Working Memory Scope</span>
            </div>
            {stats.workingMemoryKeys.length === 0 ? (
              <div className="rounded-lg border border-dashed border-border/60 bg-muted/15 px-3 py-2 text-[11px] text-muted-foreground">
                No active working memory keys in current scope.
              </div>
            ) : (
              <div className="flex flex-wrap gap-1.5">
                {stats.workingMemoryKeys.map((key) => (
                  <Badge key={key} variant="secondary" className="font-mono text-[11px]">
                    {key}
                  </Badge>
                ))}
              </div>
            )}
          </div>

          <div className="space-y-2 rounded-xl border border-border/60 bg-muted/25 p-3 font-mono text-[11px] text-muted-foreground">
            <div>
              <div className="mb-0.5 font-sans font-semibold text-foreground">
                Knowledge Graph Database
              </div>
              <div className="break-all">{stats.paths?.graphDb || 'Waiting for gateway path…'}</div>
            </div>
            <div className="border-t border-border/40 pt-2">
              <div className="mb-0.5 font-sans font-semibold text-foreground">
                Cognitive Facts Database
              </div>
              <div className="break-all">{stats.paths?.memoryDb || 'Waiting for gateway path…'}</div>
            </div>
          </div>
        </div>

        <div className="mt-6 flex items-center justify-between gap-2">
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => {
              setIsMemoryOpen(false);
              setActiveView('knowledge');
            }}
          >
            Open Graph View
          </Button>
          <Button type="button" size="sm" onClick={() => setIsMemoryOpen(false)}>
            Done
          </Button>
        </div>
      </div>
    </div>
  );
};
