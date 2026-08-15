import React, { useEffect, useState } from 'react';
import { useOpenZStore } from '../store/useOpenZStore';
import { wsService } from '../services/websocket';
import {
  ArrowLeft,
  BrainCircuit,
  Database,
  ExternalLink,
  KeyRound,
  Share2,
  FileCode,
  AlertCircle,
  Copy,
  Check,
  RefreshCw,
  Download,
  Filter,
  Search,
} from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { ObsidianGraph } from './ObsidianGraph';
import type { CognitiveNode, CognitiveEdge } from '../types/openz';

export const GraphVisualizer: React.FC<{ nodes: CognitiveNode[]; edges: CognitiveEdge[] }> = ({
  nodes,
  edges,
}) => {
  const facts = useOpenZStore((s) => s.cognitiveStats.facts || []);
  return <ObsidianGraph nodes={nodes} edges={edges} facts={facts} height={500} />;
};

function observationText(value: string): string {
  try {
    const parsed = JSON.parse(value);
    return Array.isArray(parsed) ? parsed.join(' ') : String(parsed);
  } catch {
    return value || '';
  }
}

function downloadTextFile(filename: string, content: string, mime: string) {
  const blob = new Blob([content], { type: mime });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(url);
}

export const KnowledgeView: React.FC = () => {
  const cognitiveStats = useOpenZStore((s) => s.cognitiveStats);
  const setIsMemoryOpen = useOpenZStore((s) => s.setIsMemoryOpen);
  const setActiveView = useOpenZStore((s) => s.setActiveView);

  const [activeTab, setActiveTab] = useState<'graph' | 'markdown' | 'facts'>('graph');
  const [copied, setCopied] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [nodeTypeFilter, setNodeTypeFilter] = useState('all');
  const [factTagFilter, setFactTagFilter] = useState('all');
  const [sortMode, setSortMode] = useState<'importance' | 'newest'>('importance');

  // Real-time synchronization on mount and recurring poll
  useEffect(() => {
    wsService.requestCognitiveMemory();

    const interval = setInterval(() => {
      wsService.requestCognitiveMemory();
    }, 8000);

    return () => clearInterval(interval);
  }, []);

  const handleRefresh = () => {
    setIsRefreshing(true);
    wsService.requestCognitiveMemory();
    setTimeout(() => setIsRefreshing(false), 700);
  };

  const nodeTypes = Array.from(
    new Set((cognitiveStats.nodes || []).map((node) => node.entity_type).filter(Boolean)),
  ).sort();

  const factTags = Array.from(
    new Set(
      (cognitiveStats.facts || []).flatMap((fact) =>
        fact.tags
          .split(',')
          .map((tag) => tag.trim())
          .filter(Boolean),
      ),
    ),
  ).sort();

  const normalizedSearch = searchQuery.trim().toLowerCase();
  const filteredNodes = (cognitiveStats.nodes || []).filter((node) => {
    const matchesType = nodeTypeFilter === 'all' || node.entity_type === nodeTypeFilter;
    const haystack = [node.name, node.entity_type, observationText(node.observations)].join(' ').toLowerCase();
    return matchesType && (!normalizedSearch || haystack.includes(normalizedSearch));
  });

  const filteredNodeNames = new Set(filteredNodes.map((node) => node.name));
  const filteredEdges = (cognitiveStats.edges || []).filter((edge) => {
    const endpointMatch = filteredNodeNames.has(edge.from_name) || filteredNodeNames.has(edge.to_name);
    const haystack = [edge.from_name, edge.to_name, edge.relation_type].join(' ').toLowerCase();
    return endpointMatch && (!normalizedSearch || haystack.includes(normalizedSearch) || endpointMatch);
  });

  const filteredFacts = (cognitiveStats.facts || [])
    .filter((fact) => {
      const matchesTag =
        factTagFilter === 'all' ||
        fact.tags
          .split(',')
          .map((tag) => tag.trim())
          .includes(factTagFilter);
      const haystack = [fact.text, fact.tags, fact.timestamp, String(fact.importance)].join(' ').toLowerCase();
      return matchesTag && (!normalizedSearch || haystack.includes(normalizedSearch));
    })
    .sort((a, b) =>
      sortMode === 'importance'
        ? b.importance - a.importance
        : new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime(),
    );

  const snapshotStats = {
    entitiesCount: Math.max(filteredNodes.length, cognitiveStats.entitiesCount || 0),
    relationsCount: Math.max(filteredEdges.length, cognitiveStats.relationsCount || 0),
    factsCount: Math.max(filteredFacts.length, cognitiveStats.factsCount || 0),
  };

  const generateMarkdownString = () => {
    let md = `# Knowledge Graph Memory Snapshot\n\n`;
    md += `Generated at: \`${new Date().toLocaleString()}\`\n`;
    md += `Path: \`~/.openz/memory.db\` and \`~/.openz/graph_memory.db\`\n\n`;

    md += `## 1. Node Entities (${snapshotStats.entitiesCount})\n\n`;
    if (filteredNodes.length > 0) {
      filteredNodes.forEach((n) => {
        md += `### [[${n.name}]] (${n.entity_type})\n`;
        try {
          const obs = JSON.parse(n.observations);
          if (Array.isArray(obs) && obs.length > 0) {
            obs.forEach((o) => {
              md += `- ${o}\n`;
            });
          } else {
            md += `- ${n.observations}\n`;
          }
        } catch {
          md += `- ${n.observations || 'No observations'}\n`;
        }
        md += `\n`;
      });
    } else {
      md += `*No nodes present in database.*\n\n`;
    }

    md += `## 2. Relationships (${snapshotStats.relationsCount})\n\n`;
    if (filteredEdges.length > 0) {
      md += `| Source Entity | Target Entity | Relation Type |\n`;
      md += `| :--- | :--- | :--- |\n`;
      filteredEdges.forEach((e) => {
        md += `| [[${e.from_name}]] | [[${e.to_name}]] | \`${e.relation_type}\` |\n`;
      });
      md += `\n`;
    } else {
      md += `*No edges present in database.*\n\n`;
    }

    md += `## 3. Stored Cognitive Facts (${snapshotStats.factsCount})\n\n`;
    if (filteredFacts.length > 0) {
      filteredFacts.forEach((f) => {
        md += `- **Fact**: ${f.text}\n`;
        md += `  - *Importance*: ${f.importance} | *Timestamp*: ${f.timestamp}\n`;
        if (f.tags) md += `  - *Tags*: \`${f.tags}\`\n`;
        md += `\n`;
      });
    } else {
      md += `*No long-term facts compiled yet by background curators. Learnings are saved automatically as conversation progresses or when manual memory tools are triggered.*\n\n`;
    }

    return md;
  };

  const handleCopyMarkdown = () => {
    navigator.clipboard.writeText(generateMarkdownString());
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleDownloadMarkdown = () => {
    downloadTextFile('openz-knowledge-snapshot.md', generateMarkdownString(), 'text/markdown;charset=utf-8');
  };

  const handleDownloadJson = () => {
    const payload = {
      generatedAt: new Date().toISOString(),
      filters: { searchQuery, nodeTypeFilter, factTagFilter, sortMode },
      stats: snapshotStats,
      nodes: filteredNodes,
      edges: filteredEdges,
      facts: filteredFacts,
      workingMemoryKeys: cognitiveStats.workingMemoryKeys,
    };
    downloadTextFile(
      'openz-knowledge-snapshot.json',
      JSON.stringify(payload, null, 2),
      'application/json;charset=utf-8',
    );
  };

  const cards = [
    { label: 'Entities', value: snapshotStats.entitiesCount, icon: BrainCircuit, color: 'text-amber-500' },
    { label: 'Relations', value: snapshotStats.relationsCount, icon: Share2, color: 'text-purple-400' },
    { label: 'Stored Facts', value: snapshotStats.factsCount, icon: Database, color: 'text-emerald-400' },
  ];

  return (
    <div className="mx-auto max-w-5xl px-4 py-6 space-y-6">
      {/* Header section */}
      <div className="flex items-center justify-between pb-4 border-b border-border/50">
        <div className="space-y-1">
          <button
            onClick={() => setActiveView('dashboard')}
            className="flex items-center gap-1.5 rounded-lg border border-border/60 bg-muted/20 px-3 py-1.5 text-xs font-semibold text-muted-foreground hover:text-foreground hover:bg-muted/40 transition-colors"
          >
            <ArrowLeft className="h-3.5 w-3.5" /> Go Back
          </button>
          <h1 className="flex items-center gap-2 text-2xl font-extrabold tracking-tight text-foreground pt-2">
            <BrainCircuit className="h-6 w-6 text-amber-500" /> Cognitive Memory Graph
          </h1>
          <p className="text-xs text-muted-foreground">
            Obsidian-style semantic entity-relation graph, memory clusters, and real-time cognitive index.
          </p>
        </div>

        <div className="flex items-center gap-2">
          <button
            onClick={handleRefresh}
            className="flex items-center gap-1.5 rounded-xl border border-border bg-card/80 px-3.5 py-2 text-xs font-semibold text-foreground hover:bg-muted transition duration-150 shadow-sm"
          >
            <RefreshCw className={`h-3.5 w-3.5 ${isRefreshing ? 'animate-spin text-amber-500' : ''}`} />
            Sync Graph
          </button>
        </div>
      </div>

      {/* Filter and Search Bar */}
      <div className="rounded-xl border border-border/70 bg-card/40 p-3 shadow-sm">
        <div className="grid gap-3 lg:grid-cols-[1fr_160px_160px_140px]">
          <div className="relative">
            <Search className="absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" />
            <input
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search nodes, relations, facts, tags..."
              className="w-full rounded-lg border border-border/60 bg-background py-2 pl-9 pr-3 text-xs text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-amber-500/50"
            />
          </div>
          <label className="relative">
            <Filter className="pointer-events-none absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" />
            <select
              value={nodeTypeFilter}
              onChange={(e) => setNodeTypeFilter(e.target.value)}
              className="w-full appearance-none rounded-lg border border-border/60 bg-background py-2 pl-9 pr-3 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500/50"
            >
              <option value="all">All node types</option>
              {nodeTypes.map((type) => (
                <option key={type} value={type}>
                  {type}
                </option>
              ))}
            </select>
          </label>
          <label>
            <select
              value={factTagFilter}
              onChange={(e) => setFactTagFilter(e.target.value)}
              className="w-full rounded-lg border border-border/60 bg-background px-3 py-2 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500/50"
            >
              <option value="all">All fact tags</option>
              {factTags.map((tag) => (
                <option key={tag} value={tag}>
                  {tag}
                </option>
              ))}
            </select>
          </label>
          <label>
            <select
              value={sortMode}
              onChange={(e) => setSortMode(e.target.value as 'importance' | 'newest')}
              className="w-full rounded-lg border border-border/60 bg-background px-3 py-2 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500/50"
            >
              <option value="importance">Importance</option>
              <option value="newest">Newest</option>
            </select>
          </label>
        </div>
        <div className="mt-2 flex flex-wrap items-center gap-2 text-[10px] text-muted-foreground">
          <span className="rounded bg-muted/50 px-2 py-0.5 font-mono">nodes: {snapshotStats.entitiesCount}</span>
          <span className="rounded bg-muted/50 px-2 py-0.5 font-mono">{snapshotStats.relationsCount} relations</span>
          <span className="rounded bg-muted/50 px-2 py-0.5 font-mono">{snapshotStats.factsCount} facts</span>
          {(searchQuery || nodeTypeFilter !== 'all' || factTagFilter !== 'all') && (
            <button
              type="button"
              onClick={() => {
                setSearchQuery('');
                setNodeTypeFilter('all');
                setFactTagFilter('all');
              }}
              className="ml-auto rounded border border-border/60 px-2 py-0.5 font-semibold text-muted-foreground transition hover:bg-muted hover:text-foreground"
            >
              Clear filters
            </button>
          )}
        </div>
      </div>

      {/* Metrics Row */}
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
        {cards.map((card) => {
          const Icon = card.icon;
          return (
            <div key={card.label} className="rounded-xl border border-border bg-card p-4 shadow-sm">
              <div className="flex items-center justify-between text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
                {card.label}
                <Icon className={`h-4 w-4 ${card.color}`} />
              </div>
              <div className="mt-1.5 text-2xl font-extrabold tracking-tight text-foreground">{card.value}</div>
            </div>
          );
        })}
      </div>

      {/* Mode Selector Tab Row */}
      <div className="flex items-center justify-between border-b border-border/40 pb-2">
        <div className="flex gap-1.5">
          <button
            onClick={() => setActiveTab('graph')}
            className={`rounded-xl px-4 py-2 text-xs font-semibold transition-all ${
              activeTab === 'graph'
                ? 'bg-amber-500/15 text-amber-400 border border-amber-500/30 shadow-sm'
                : 'text-muted-foreground hover:bg-muted hover:text-foreground'
            }`}
          >
            Obsidian Graph View
          </button>
          <button
            onClick={() => setActiveTab('markdown')}
            className={`rounded-xl px-4 py-2 text-xs font-semibold transition-all ${
              activeTab === 'markdown'
                ? 'bg-amber-500/15 text-amber-400 border border-amber-500/30 shadow-sm'
                : 'text-muted-foreground hover:bg-muted hover:text-foreground'
            }`}
          >
            Markdown Snapshot (.md)
          </button>
          <button
            onClick={() => setActiveTab('facts')}
            className={`rounded-xl px-4 py-2 text-xs font-semibold transition-all ${
              activeTab === 'facts'
                ? 'bg-amber-500/15 text-amber-400 border border-amber-500/30 shadow-sm'
                : 'text-muted-foreground hover:bg-muted hover:text-foreground'
            }`}
          >
            Stored Facts List
          </button>
        </div>

        <div className="flex items-center gap-2">
          {activeTab === 'markdown' && (
            <button
              onClick={handleCopyMarkdown}
              className="flex items-center gap-1.5 rounded-lg border border-border/80 bg-muted/20 px-3 py-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors"
            >
              {copied ? (
                <>
                  <Check className="h-3.5 w-3.5 text-emerald-500" /> Copied!
                </>
              ) : (
                <>
                  <Copy className="h-3.5 w-3.5" /> Copy Markdown
                </>
              )}
            </button>
          )}
          <button
            onClick={handleDownloadMarkdown}
            className="flex items-center gap-1.5 rounded-lg border border-border/80 bg-muted/20 px-3 py-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors"
          >
            <Download className="h-3.5 w-3.5" /> MD
          </button>
          <button
            onClick={handleDownloadJson}
            className="flex items-center gap-1.5 rounded-lg border border-border/80 bg-muted/20 px-3 py-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors"
          >
            <Download className="h-3.5 w-3.5" /> JSON
          </button>
        </div>
      </div>

      {/* Main Tab Panels */}
      <div className="min-h-[480px]">
        {activeTab === 'graph' && (
          <div className="space-y-3">
            <ObsidianGraph
              nodes={filteredNodes}
              edges={filteredEdges}
              facts={filteredFacts}
              height={520}
            />
            <div className="flex items-center justify-between text-[11px] text-muted-foreground px-1 select-none">
              <span>💡 Drag nodes to interact. Scroll wheel to zoom in/out. Click background to pan.</span>
              <span className="font-mono text-emerald-400 flex items-center gap-1">
                <span className="h-2 w-2 rounded-full bg-emerald-500 animate-pulse" /> Live Realtime Sync Active
              </span>
            </div>
          </div>
        )}

        {activeTab === 'markdown' && (
          <div className="rounded-2xl border border-border bg-zinc-950/90 shadow-2xl overflow-hidden flex flex-col max-h-[550px]">
            {/* Editor Top Bar */}
            <div className="flex items-center gap-1.5 border-b border-border/60 bg-muted/20 px-4 py-2.5 select-none">
              <FileCode className="h-4 w-4 text-amber-500" />
              <span className="font-mono text-[11px] text-muted-foreground">cognitive_graph_snapshot.md</span>
              <span className="ml-auto rounded bg-emerald-500/10 border border-emerald-500/20 px-1.5 py-0.5 text-[9px] text-emerald-400 font-semibold font-mono">
                READ-ONLY
              </span>
            </div>
            {/* Editor Body */}
            <div className="flex-1 overflow-y-auto p-5 text-xs font-mono prose prose-invert max-w-none text-foreground prose-xs scrollbar-thin">
              <ReactMarkdown remarkPlugins={[remarkGfm]}>{generateMarkdownString()}</ReactMarkdown>
            </div>
          </div>
        )}

        {activeTab === 'facts' && (
          <div className="space-y-4">
            {filteredFacts.length === 0 && (
              <div className="rounded-2xl border border-amber-500/15 bg-amber-500/5 p-4 flex gap-3 text-xs leading-relaxed">
                <AlertCircle className="h-5 w-5 text-amber-500 shrink-0 mt-0.5" />
                <div className="space-y-2">
                  <div className="font-semibold text-amber-400">Why are my Stored Facts showing 0?</div>
                  <p className="text-muted-foreground text-[11px]">
                    OpenZ utilizes an asynchronous <strong>Self-Improvement Memory Curator</strong> that runs in the background after turns. It compiles conversation observations into long-term facts in SQLite (<code>~/.openz/memory.db</code>).
                  </p>
                  <p className="text-muted-foreground text-[11px]">
                    You can also store facts directly using native memory tools like <code>extract_and_store_facts</code>, <code>smart_store</code>, or <code>set_working_memory</code>.
                  </p>
                </div>
              </div>
            )}

            {/* List of facts */}
            <div className="space-y-2 max-h-[500px] overflow-y-auto pr-1">
              {filteredFacts.length === 0 ? (
                <div className="rounded-2xl border border-border/50 bg-card/40 p-12 text-center text-xs text-muted-foreground select-none">
                  No facts in cognitive database. Start chatting or execute memory tools to store memories!
                </div>
              ) : (
                filteredFacts.map((fact, idx) => (
                  <div
                    key={idx}
                    className="rounded-xl border border-border bg-card p-4 shadow-sm hover:border-amber-500/40 transition-colors"
                  >
                    <div className="flex items-center gap-2 mb-1.5">
                      <span className="rounded bg-amber-500/10 border border-amber-500/20 px-1.5 py-0.5 text-[9px] text-amber-400 font-semibold font-mono">
                        Importance: {fact.importance.toFixed(2)}
                      </span>
                      {fact.tags && (
                        <span className="rounded bg-muted px-1.5 py-0.5 text-[9px] text-muted-foreground font-mono truncate max-w-[250px]">
                          Tags: {fact.tags}
                        </span>
                      )}
                      <span className="ml-auto text-[9px] text-muted-foreground font-mono">{fact.timestamp}</span>
                    </div>
                    <p className="text-xs text-foreground leading-relaxed break-words font-sans">{fact.text}</p>
                  </div>
                ))
              )}
            </div>
          </div>
        )}
      </div>

      {/* Active Working Memory scope */}
      <div className="rounded-2xl border border-border bg-card p-4 shadow-sm">
        <div className="flex items-center gap-2 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
          <KeyRound className="h-4 w-4 text-amber-500" /> Working Memory Scope
          <span className="ml-auto rounded-full bg-muted/70 px-2 py-0.5 font-mono text-[10px]">
            {cognitiveStats.workingMemoryKeys.length}
          </span>
        </div>
        {cognitiveStats.workingMemoryKeys.length === 0 ? (
          <div className="mt-3 text-xs text-muted-foreground/70 select-none">
            No working memory keys scope set. The agent will populate them dynamically.
          </div>
        ) : (
          <div className="mt-3 flex flex-wrap gap-2">
            {cognitiveStats.workingMemoryKeys.map((key) => (
              <span
                key={key}
                className="rounded-lg border border-border bg-muted/30 px-2.5 py-1 font-mono text-[10px] text-foreground"
              >
                {key}
              </span>
            ))}
          </div>
        )}
      </div>

      {/* Inspector control */}
      <div className="flex justify-end pt-2">
        <button
          onClick={() => {
            setIsMemoryOpen(true);
            wsService.requestCognitiveMemory();
          }}
          className="flex items-center gap-2 rounded-xl border border-amber-500/30 bg-amber-500/10 px-4 py-2.5 text-xs font-semibold text-amber-400 transition hover:bg-amber-500/20 shadow-sm"
        >
          <ExternalLink className="h-4 w-4" /> Open full memory inspector modal
        </button>
      </div>
    </div>
  );
};