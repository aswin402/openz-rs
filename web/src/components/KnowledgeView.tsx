import React from 'react';
import { useOpenZStore } from '../store/useOpenZStore';
import { wsService } from '../services/websocket';
import { ArrowLeft, BrainCircuit, Database, ExternalLink, KeyRound, Share2 } from 'lucide-react';

export const KnowledgeView: React.FC = () => {
  const cognitiveStats = useOpenZStore((s) => s.cognitiveStats);
  const setIsMemoryOpen = useOpenZStore((s) => s.setIsMemoryOpen);
  const setActiveView = useOpenZStore((s) => s.setActiveView);

  const cards = [
    { label: 'Entities', value: cognitiveStats.entitiesCount, icon: BrainCircuit },
    { label: 'Relations', value: cognitiveStats.relationsCount, icon: Share2 },
    { label: 'Facts', value: cognitiveStats.factsCount, icon: Database },
  ];

  return (
    <div className="mx-auto max-w-4xl px-4 py-6">
      {/* Back button */}
      <div className="mb-4">
        <button
          onClick={() => setActiveView('dashboard')}
          className="flex items-center gap-1.5 rounded-lg border border-border/60 bg-muted/20 px-3 py-1.5 text-xs font-semibold text-muted-foreground hover:text-foreground hover:bg-muted/40 transition-colors"
        >
          <ArrowLeft className="h-3.5 w-3.5" /> Go Back
        </button>
      </div>

      <h1 className="flex items-center gap-2 text-2xl font-extrabold tracking-tight text-foreground">
        <BrainCircuit className="h-5 w-5 text-amber-500" /> Knowledge Graph
      </h1>
      <p className="mt-1 text-sm text-muted-foreground">
        Semantic + graph memory state, streamed live from the gateway.
      </p>

      <div className="mt-6 grid grid-cols-1 gap-3 sm:grid-cols-3">
        {cards.map((card) => {
          const Icon = card.icon;
          return (
            <div key={card.label} className="rounded-xl border border-border/60 bg-card/60 p-4 shadow-sm">
              <div className="flex items-center justify-between text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
                {card.label}
                <Icon className="h-4 w-4 text-amber-500" />
              </div>
              <div className="mt-1.5 text-2xl font-extrabold tracking-tight text-foreground">{card.value}</div>
            </div>
          );
        })}
      </div>

      {/* Working memory (real keys from the backend) */}
      <div className="mt-6 rounded-xl border border-border/60 bg-card/60 p-4 shadow-sm">
        <div className="flex items-center gap-2 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
          <KeyRound className="h-4 w-4 text-amber-500" /> Working Memory Keys
          <span className="ml-auto rounded-full bg-muted/70 px-2 py-0.5 font-mono text-[10px]">
            {cognitiveStats.workingMemoryKeys.length}
          </span>
        </div>
        {cognitiveStats.workingMemoryKeys.length === 0 ? (
          <div className="mt-3 text-xs text-muted-foreground/70">
            No working memory keys set. The agent will populate them as it works.
          </div>
        ) : (
          <div className="mt-3 flex flex-wrap gap-2">
            {cognitiveStats.workingMemoryKeys.map((key) => (
              <span
                key={key}
                className="rounded-md border border-border/60 bg-muted/40 px-2 py-1 font-mono text-[11px] text-foreground"
              >
                {key}
              </span>
            ))}
          </div>
        )}
      </div>

      <button
        onClick={() => {
          setIsMemoryOpen(true);
          wsService.requestCognitiveMemory();
        }}
        className="mt-6 flex items-center gap-2 rounded-xl border border-amber-500/30 bg-amber-500/10 px-4 py-2 text-xs font-semibold text-amber-400 transition hover:bg-amber-500/20"
      >
        <ExternalLink className="h-4 w-4" /> Open full memory inspector
      </button>
    </div>
  );
};