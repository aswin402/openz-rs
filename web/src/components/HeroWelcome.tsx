import React from 'react';
import { useOpenZStore } from '../store/useOpenZStore';
import { Code2, Search, Brain, FileText } from 'lucide-react';

export const HeroWelcome: React.FC = () => {
  const sendMessage = useOpenZStore((s) => s.sendMessage);
  const activeModel = useOpenZStore((s) => s.activeModel);
  const status = useOpenZStore((s) => s.status);

  const promptCards = [
    {
      title: 'Analyze Codebase',
      desc: 'AST structural search + cargo check on your project',
      prompt: 'Use ast_grep and cargo check to analyze the codebase architecture and report findings.',
      icon: Code2,
    },
    {
      title: 'Web & Deep Research',
      desc: 'Multi-engine web search with browser fallback',
      prompt: 'Search the web for the latest updates on Rust async programming and summarize findings.',
      icon: Search,
    },
    {
      title: 'Cognitive Memory',
      desc: 'Inspect knowledge graph entities, relations & working memory',
      prompt: '/memory',
      icon: Brain,
    },
    {
      title: 'Document Processing',
      desc: 'Extract text and tables from PDF/DOCX files',
      prompt: 'Extract content from PDF documents in the project and summarize.',
      icon: FileText,
    },
  ];

  return (
    <div className="flex flex-col items-center justify-center py-12 px-4 text-center">


      <h1 className="text-3xl font-extrabold tracking-tight text-foreground sm:text-4xl">
        OpenZ <span className="bg-gradient-to-r from-amber-500 to-orange-500 bg-clip-text text-transparent">Agent 🦊</span>
      </h1>
      <p className="mt-2 max-w-md text-sm text-muted-foreground">
        High-performance async personal AI agent framework in Rust. Powered by native tools & durable
        memory{status?.version ? ` · v${status.version}` : ''}
        {activeModel ? ` · ${activeModel}` : ''}.
      </p>

      <div className="mt-8 grid w-full max-w-2xl grid-cols-1 gap-3 sm:grid-cols-2">
        {promptCards.map((card) => {
          const Icon = card.icon;
          return (
            <button
              key={card.title}
              onClick={() => sendMessage(card.prompt)}
              className="flex flex-col items-start p-4 rounded-xl border border-border/60 bg-card/60 hover:bg-card hover:border-amber-500/40 text-left transition-all group shadow-sm"
            >
              <div className="flex items-center gap-2 text-xs font-semibold text-foreground group-hover:text-amber-500 transition">
                <Icon className="h-4 w-4 text-amber-500" />
                {card.title}
              </div>
              <p className="mt-1 text-xs text-muted-foreground/80">{card.desc}</p>
            </button>
          );
        })}
      </div>
    </div>
  );
};
