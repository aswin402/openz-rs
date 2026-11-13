import React, { useMemo } from 'react';
import { useOpenZStore } from '../store/useOpenZStore';
import { useUiCustomizationStore } from '../store/useUiCustomizationStore';
import { ChatTrackRow } from './ChatTrackRow';
import { OpenZBrandLogo } from './OpenZFoxIcon';
import {
  BeuiPromptSuggestions,
  type BeuiPromptSuggestionItem,
} from './beui';
import { Badge } from './ui/badge';
import { Code2, Search, Brain, FileText, Palette } from 'lucide-react';

export const HeroWelcome: React.FC = () => {
  const sendMessage = useOpenZStore((s) => s.sendMessage);
  const activeModel = useOpenZStore((s) => s.activeModel);
  const status = useOpenZStore((s) => s.status);
  const runtimeInventory = useOpenZStore((s) => s.runtimeInventory);
  const cognitiveStats = useOpenZStore((s) => s.cognitiveStats);
  const mcpStats = useOpenZStore((s) => s.mcpStats);
  const chatWidth = useUiCustomizationStore((s) => s.chatWidth);
  const promptSuggestionsStyle = useUiCustomizationStore((s) => s.promptSuggestionsStyle);
  const setIsCustomizeOpen = useUiCustomizationStore((s) => s.setIsCustomizeOpen);

  const suggestions = useMemo<BeuiPromptSuggestionItem[]>(() => {
    const toolCount = runtimeInventory?.counts.tools;
    const entityCount = cognitiveStats?.entitiesCount;
    const relationCount = cognitiveStats?.relationsCount;
    const mcpLoaded = mcpStats.loaded;

    return [
      {
        id: 'analyze-codebase',
        title: 'Analyze Codebase',
        description: toolCount
          ? `AST structural search across ${toolCount} registered native tools`
          : 'AST structural search + focused package check on your project',
        prompt:
          'Use ast_grep and just check openz to analyze the codebase architecture and report focused findings. Do not run full cargo check, build, or test commands.',
        icon: Code2,
      },
      {
        id: 'web-research',
        title: 'Web & Deep Research',
        description:
          mcpLoaded > 0
            ? `Multi-engine search + ${mcpLoaded} loaded MCP servers with @beui/citations`
            : 'Multi-engine web search with inline @beui/citations sources',
        prompt:
          'Search the web for the latest updates on Rust async programming and summarize findings.',
        icon: Search,
      },
      {
        id: 'cognitive-memory',
        title: 'Cognitive Memory',
        description:
          typeof entityCount === 'number' && typeof relationCount === 'number'
            ? `Inspect ${entityCount} graph entities & ${relationCount} relations in memory`
            : 'Inspect knowledge graph entities, relations & working memory',
        prompt: '/memory',
        icon: Brain,
      },
      {
        id: 'doc-processing',
        title: 'Document Processing',
        description: 'Extract structured text, tables & metadata from PDF/DOCX/XLSX files',
        prompt: 'Extract content from PDF documents in the project and summarize.',
        icon: FileText,
      },
    ];
  }, [cognitiveStats, mcpStats.loaded, runtimeInventory]);

  return (
    <ChatTrackRow chatWidth={chatWidth} className="py-12">
      <div className="flex w-full flex-col items-center justify-center text-center">
        {/* Inline OpenZ Agent <FoxSilhouette> Logo (Image 1) — No upper fox icon */}
        <h1>
          <OpenZBrandLogo size="lg" />
        </h1>
        <p className="mt-2.5 max-w-lg text-sm text-muted-foreground">
          High-performance async personal AI agent framework in Rust. Powered by native tools,
          shadcn/ui &amp; @beui components.
        </p>

        <div className="mt-3.5 flex flex-wrap items-center justify-center gap-1.5">
          {status?.version && (
            <Badge variant="outline" className="font-mono text-[10px]">
              v{status.version}
            </Badge>
          )}
          {activeModel && (
            <Badge variant="secondary" className="font-mono text-[10px]">
              {activeModel}
            </Badge>
          )}
          <button
            type="button"
            onClick={() => setIsCustomizeOpen(true)}
            className="inline-flex items-center gap-1 rounded-md border border-primary/35 bg-primary/10 px-2 py-0.5 text-[11px] font-medium text-primary transition-colors hover:bg-primary/20"
          >
            <Palette className="size-3" />
            <span>Customize UI</span>
          </button>
        </div>

        <div className="mt-8 w-full">
          <BeuiPromptSuggestions
            items={suggestions}
            variant={promptSuggestionsStyle}
            onSelect={(prompt) => sendMessage(prompt)}
          />
        </div>
      </div>
    </ChatTrackRow>
  );
};
