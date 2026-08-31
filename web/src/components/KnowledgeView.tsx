import React, { useEffect, useMemo, useState } from 'react';
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
  X,
} from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { ObsidianGraph } from './ObsidianGraph';
import type { CognitiveNode, CognitiveEdge } from '../types/openz';
import { cn } from '../lib/utils';
import type { GraphMode } from './graphLayout';
import { buildGraphSemantics, edgeKey, formatConfidence, formatProvenance, type EdgeSemantics } from './graphSemantics';

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

const MemoryEntityInspector: React.FC<{
  node: CognitiveNode | null;
  edges: CognitiveEdge[];
  neighbors: CognitiveNode[];
  edgeSemantics: Map<string, EdgeSemantics>;
  selectedEdgeKey: string | null;
  onSelectEdge: (key: string) => void;
  onFocusNeighborhood: () => void;
  onClear: () => void;
}> = ({ node, edges, neighbors, edgeSemantics, selectedEdgeKey, onSelectEdge, onFocusNeighborhood, onClear }) => {
  if (!node) {
    return (
      <aside className="flex min-h-[220px] flex-col justify-center rounded-2xl border border-dashed border-border/70 bg-card/30 p-5 text-center">
        <BrainCircuit className="mx-auto h-7 w-7 text-muted-foreground/40" />
        <h2 className="mt-3 text-sm font-semibold text-foreground">Select an entity</h2>
        <p className="mt-1 text-xs leading-relaxed text-muted-foreground">
          Click a node or cluster to inspect its persisted observations and relationships.
        </p>
      </aside>
    );
  }

  return (
    <aside aria-label="Selected memory entity" className="flex min-h-[220px] flex-col rounded-2xl border border-amber-500/20 bg-card/60 p-4 shadow-sm">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex items-center gap-2 text-[10px] font-semibold uppercase tracking-wider text-amber-400">
            <span className="h-2 w-2 rounded-full bg-amber-400" />
            {node.entity_type}
          </div>
          <h2 className="mt-1 break-words text-base font-bold text-foreground">{node.name}</h2>
        </div>
        <button type="button" onClick={onClear} className="rounded-lg p-1.5 text-muted-foreground transition hover:bg-muted hover:text-foreground" aria-label="Clear selected entity">
          <X className="h-4 w-4" />
        </button>
      </div>
      <div className="mt-4 rounded-xl border border-border/60 bg-background/40 p-3">
        <div className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">Observations</div>
        <p className="mt-2 max-h-32 overflow-y-auto whitespace-pre-wrap break-words text-xs leading-relaxed text-foreground/80">
          {observationText(node.observations) || 'No observations recorded.'}
        </p>
      </div>
      <div className="mt-3 grid grid-cols-2 gap-2">
        <div className="rounded-xl bg-muted/40 p-2.5">
          <div className="text-[10px] text-muted-foreground">Relations</div>
          <div className="mt-1 text-lg font-bold text-foreground">{edges.length}</div>
        </div>
        <div className="rounded-xl bg-muted/40 p-2.5">
          <div className="text-[10px] text-muted-foreground">Neighbors</div>
          <div className="mt-1 text-lg font-bold text-foreground">{neighbors.length}</div>
        </div>
      </div>
      <div className="mt-3 min-h-0 flex-1 space-y-1 overflow-y-auto">
        {edges.slice(0, 12).map((edge) => {
          const relationKey = edgeKey(edge);
          const detail = edgeSemantics.get(relationKey);
          const neighbor = edge.from_name === node.name ? edge.to_name : edge.from_name;
          const isSelected = selectedEdgeKey === relationKey;
          return (
            <button
              key={relationKey}
              type="button"
              onClick={() => onSelectEdge(relationKey)}
              className={cn('w-full rounded-lg border px-2.5 py-2 text-left text-[10px] transition', isSelected ? 'border-cyan-400/40 bg-cyan-400/10' : 'border-border/50 hover:border-amber-500/30 hover:bg-muted/40')}
              aria-pressed={isSelected}
            >
              <div className="flex items-center justify-between gap-2">
                <span className="min-w-0 truncate font-medium text-foreground">{edge.from_name} → {edge.to_name}</span>
                <span className="shrink-0 rounded bg-amber-500/10 px-1.5 py-0.5 font-mono text-amber-400">{edge.relation_type}</span>
              </div>
              <div className="mt-1.5 flex flex-wrap gap-x-2 gap-y-1 font-mono text-[9px] text-muted-foreground">
                <span>Neighbor: {neighbor}</span>
                <span>Confidence: {formatConfidence(detail?.confidence ?? null)}</span>
                <span>Provenance: {formatProvenance(detail?.provenance ?? 'not_recorded')}</span>
                <span>Source: {detail?.source ?? 'Not recorded'}</span>
              </div>
            </button>
          );
        })}
        {edges.length === 0 && <div className="py-3 text-xs text-muted-foreground">No persisted relationships for this entity.</div>}
      </div>
      <button type="button" onClick={onFocusNeighborhood} className="mt-3 min-h-10 rounded-xl border border-amber-500/25 bg-amber-500/10 px-3 text-xs font-semibold text-amber-300 transition hover:bg-amber-500/20">
        Focus this neighborhood
      </button>
    </aside>
  );
};

export const KnowledgeView: React.FC = () => {
  const cognitiveStats = useOpenZStore((s) => s.cognitiveStats);
  const setIsMemoryOpen = useOpenZStore((s) => s.setIsMemoryOpen);
  const setActiveView = useOpenZStore((s) => s.setActiveView);
  const runtimeInventory = useOpenZStore((s) => s.runtimeInventory);

  const [activeTab, setActiveTab] = useState<'graph' | 'markdown' | 'facts'>('graph');
  const [copied, setCopied] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [nodeTypeFilter, setNodeTypeFilter] = useState('all');
  const [factTagFilter, setFactTagFilter] = useState('all');
  const [sortMode, setSortMode] = useState<'importance' | 'newest'>('importance');
  const [graphMode, setGraphMode] = useState<GraphMode>('overview');
  const [selectedNodeName, setSelectedNodeName] = useState<string | null>(null);
  const [selectedEdgeKey, setSelectedEdgeKey] = useState<string | null>(null);
  const [communityFilter, setCommunityFilter] = useState('all');
  const [relationTypeFilter, setRelationTypeFilter] = useState('all');
  const [mostConnectedOnly, setMostConnectedOnly] = useState(false);
  const [visibleGraphStats, setVisibleGraphStats] = useState({
    loaded: 0,
    visible: 0,
    edges: 0,
    renderedNodes: 0,
    renderedEdges: 0,
  });

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

  const allNodes = useMemo(() => cognitiveStats.nodes || [], [cognitiveStats.nodes]);
  const allEdges = useMemo(() => cognitiveStats.edges || [], [cognitiveStats.edges]);
  const graphSemantics = useMemo(() => buildGraphSemantics(allNodes, allEdges), [allEdges, allNodes]);
  const communityOptions = useMemo(
    () => graphSemantics.clusterIds.map((id, index) => ({ id, label: `Constellation ${String(index + 1).padStart(2, '0')}` })),
    [graphSemantics.clusterIds],
  );
  const relationTypes = useMemo(
    () => Array.from(new Set(allEdges.map((edge) => edge.relation_type).filter(Boolean))).sort(),
    [allEdges],
  );
  const degreeThreshold = useMemo(() => {
    const degrees = Array.from(graphSemantics.nodeMetrics.values()).map((metric) => metric.degree).sort((left, right) => right - left);
    return degrees.length === 0 ? 0 : degrees[Math.min(degrees.length - 1, Math.floor(degrees.length * 0.25))];
  }, [graphSemantics]);

  const normalizedSearch = searchQuery.trim().toLowerCase();
  const matchingEdgeEndpoints = useMemo(() => {
    if (!normalizedSearch) return new Set<string>();
    return new Set(
      allEdges
        .filter((edge) => [edge.from_name, edge.to_name, edge.relation_type].join(' ').toLowerCase().includes(normalizedSearch))
        .flatMap((edge) => [edge.from_name, edge.to_name]),
    );
  }, [allEdges, normalizedSearch]);

  const filteredNodes = allNodes.filter((node) => {
    const metric = graphSemantics.nodeMetrics.get(node.name);
    const matchesType = nodeTypeFilter === 'all' || node.entity_type === nodeTypeFilter;
    const matchesCommunity = communityFilter === 'all' || metric?.clusterId === communityFilter;
    const matchesConnected = !mostConnectedOnly || (metric?.degree || 0) >= degreeThreshold;
    const haystack = [node.name, node.entity_type, observationText(node.observations)].join(' ').toLowerCase();
    return matchesType && matchesCommunity && matchesConnected && (!normalizedSearch || haystack.includes(normalizedSearch) || matchingEdgeEndpoints.has(node.name));
  });

  const filteredNodeNames = new Set(filteredNodes.map((node) => node.name));
  const filteredEdges = allEdges.filter((edge) => {
    const endpointMatch = filteredNodeNames.has(edge.from_name) || filteredNodeNames.has(edge.to_name);
    const matchesRelationType = relationTypeFilter === 'all' || edge.relation_type === relationTypeFilter;
    const haystack = [edge.from_name, edge.to_name, edge.relation_type].join(' ').toLowerCase();
    const relationMatch = !normalizedSearch || haystack.includes(normalizedSearch);
    const nodeMatch = !normalizedSearch || filteredNodeNames.has(edge.from_name) || filteredNodeNames.has(edge.to_name);
    return endpointMatch && matchesRelationType && (relationMatch || nodeMatch);
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
    entitiesCount: filteredNodes.length,
    relationsCount: filteredEdges.length,
    factsCount: filteredFacts.length,
  };

  const totalStats = {
    entitiesCount: cognitiveStats.entitiesCount || 0,
    relationsCount: cognitiveStats.relationsCount || 0,
    factsCount: cognitiveStats.factsCount || 0,
  };
  const selectedNode = selectedNodeName
    ? allNodes.find((node) => node.name === selectedNodeName) || null
    : null;
  const selectedNodeEdges = selectedNode
    ? allEdges.filter((edge) => edge.from_name === selectedNode.name || edge.to_name === selectedNode.name)
    : [];
  const selectedNeighborNames = new Set(
    selectedNodeEdges.flatMap((edge) => [edge.from_name, edge.to_name]).filter((name) => name !== selectedNode?.name),
  );
  const selectedNeighbors = allNodes.filter((node) => selectedNeighborNames.has(node.name));
  const graphNodes = selectedNode && !filteredNodes.some((node) => node.name === selectedNode.name)
    ? [...filteredNodes, selectedNode]
    : filteredNodes;
  const graphNodeNames = new Set(graphNodes.map((node) => node.name));
  const graphEdges = filteredEdges.filter((edge) => graphNodeNames.has(edge.from_name) && graphNodeNames.has(edge.to_name));
  const selectedEdge = selectedEdgeKey ? allEdges.find((edge) => edgeKey(edge) === selectedEdgeKey) || null : null;
  const handleSelectNode = (nodeName: string | null) => {
    setSelectedNodeName(nodeName);
    setSelectedEdgeKey(null);
  };
  const handleSelectEdge = (key: string | null) => {
    setSelectedEdgeKey(key);
    if (!key) return;
    const edge = allEdges.find((candidate) => edgeKey(candidate) === key);
    if (edge) setSelectedNodeName(edge.from_name);
  };
  const graphDbExists = runtimeInventory?.memory.graphDb.exists ?? Boolean(cognitiveStats.paths?.graphDb);
  const memoryDbExists = runtimeInventory?.memory.memoryDb.exists ?? Boolean(cognitiveStats.paths?.memoryDb);
  const syncLabel = allNodes.length > 0 ? 'Live now' : 'Waiting for gateway';
  const memoryDbPath = cognitiveStats.paths?.memoryDb || runtimeInventory?.paths.memoryDb || 'Runtime path unavailable';
  const graphDbPath = cognitiveStats.paths?.graphDb || runtimeInventory?.paths.graphDb || 'Runtime path unavailable';

  const generateMarkdownString = () => {
    let md = `# Knowledge Graph Memory Snapshot\n\n`;
    md += `Generated at: \`${new Date().toLocaleString()}\`\n`;
    md += `Memory DB: \`${memoryDbPath}\`\n`;
    md += `Graph DB: \`${graphDbPath}\`\n\n`;

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
      filters: { searchQuery, nodeTypeFilter, communityFilter, relationTypeFilter, mostConnectedOnly, factTagFilter, sortMode },
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
    <div className="mx-auto max-w-6xl space-y-6 px-4 py-6">
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

      <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        <div className="rounded-2xl border border-border/70 bg-card/50 p-3 shadow-sm">
          <div className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">Loaded entities</div>
          <div className="mt-1 text-xl font-extrabold text-foreground">{totalStats.entitiesCount}</div>
          <div className="text-[10px] text-muted-foreground">{visibleGraphStats.loaded > 0 ? visibleGraphStats.visible : 0} visible now</div>
        </div>
        <div className="rounded-2xl border border-border/70 bg-card/50 p-3 shadow-sm">
          <div className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">Relations</div>
          <div className="mt-1 text-xl font-extrabold text-foreground">{totalStats.relationsCount}</div>
          <div className="text-[10px] text-muted-foreground">{visibleGraphStats.loaded > 0 ? visibleGraphStats.renderedEdges : 0} rendered now</div>
        </div>
        <div className="rounded-2xl border border-border/70 bg-card/50 p-3 shadow-sm">
          <div className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">Memory stores</div>
          <div className="mt-1 flex items-center gap-2 text-sm font-bold text-foreground">
            <span className={cn('h-2 w-2 rounded-full', memoryDbExists ? 'bg-emerald-400' : 'bg-red-400')} />
            {memoryDbExists ? 'Memory ready' : 'Memory unavailable'}
          </div>
          <div className="mt-1 text-[10px] text-muted-foreground">{graphDbExists ? 'Graph database ready' : 'Graph database unavailable'}</div>
        </div>
        <div className="rounded-2xl border border-border/70 bg-card/50 p-3 shadow-sm">
          <div className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">Last sync</div>
          <div className="mt-1 text-sm font-bold text-foreground">{syncLabel}</div>
          <div className="text-[10px] text-muted-foreground">{allNodes.length > 0 ? 'Authoritative gateway payload' : 'No records received'}</div>
        </div>
      </div>

      {/* Filter and Search Bar */}
      <div className="rounded-xl border border-border/70 bg-card/40 p-3 shadow-sm">
        <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-7">
          <div className="relative xl:col-span-2">
            <Search className="absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" />
            <input
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search nodes, relations, facts, tags..."
              className="min-h-10 w-full rounded-lg border border-border/60 bg-background py-2 pl-9 pr-3 text-xs text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-amber-500/50"
            />
          </div>
          <label className="relative">
            <Filter className="pointer-events-none absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" />
            <select
              value={nodeTypeFilter}
              onChange={(e) => setNodeTypeFilter(e.target.value)}
              className="min-h-10 w-full appearance-none rounded-lg border border-border/60 bg-background py-2 pl-9 pr-3 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500/50"
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
              value={communityFilter}
              onChange={(e) => setCommunityFilter(e.target.value)}
              className="min-h-10 w-full appearance-none rounded-lg border border-border/60 bg-background px-3 py-2 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500/50"
            >
              <option value="all">All constellations</option>
              {communityOptions.map((community) => (
                <option key={community.id} value={community.id}>
                  {community.label}
                </option>
              ))}
            </select>
          </label>
          <label>
            <select
              value={relationTypeFilter}
              onChange={(e) => setRelationTypeFilter(e.target.value)}
              className="min-h-10 w-full appearance-none rounded-lg border border-border/60 bg-background px-3 py-2 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500/50"
            >
              <option value="all">All relations</option>
              {relationTypes.map((relation) => (
                <option key={relation} value={relation}>
                  {relation}
                </option>
              ))}
            </select>
          </label>
          <label>
            <select
              value={factTagFilter}
              onChange={(e) => setFactTagFilter(e.target.value)}
              className="min-h-10 w-full rounded-lg border border-border/60 bg-background px-3 py-2 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500/50"
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
              className="min-h-10 w-full rounded-lg border border-border/60 bg-background px-3 py-2 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500/50"
            >
              <option value="importance">Importance</option>
              <option value="newest">Newest</option>
            </select>
          </label>
        </div>
        <div className="mt-2 flex flex-wrap items-center gap-2 text-[10px] text-muted-foreground">
          <label className="flex min-h-7 cursor-pointer items-center gap-1.5 rounded border border-border/60 bg-muted/20 px-2 py-0.5 font-semibold text-foreground/80">
            <input type="checkbox" checked={mostConnectedOnly} onChange={(e) => setMostConnectedOnly(e.target.checked)} className="h-3.5 w-3.5 accent-amber-500" />
            Most connected
          </label>
          <span className="rounded bg-muted/50 px-2 py-0.5 font-mono">nodes: {snapshotStats.entitiesCount}</span>
          <span className="rounded bg-muted/50 px-2 py-0.5 font-mono">{snapshotStats.relationsCount} relations</span>
          <span className="rounded bg-muted/50 px-2 py-0.5 font-mono">{snapshotStats.factsCount} facts</span>
          <span className="rounded bg-muted/30 px-2 py-0.5 font-mono text-muted-foreground/80">
            backend total: {totalStats.entitiesCount} nodes / {totalStats.relationsCount} relations / {totalStats.factsCount} facts
          </span>
          {(searchQuery || nodeTypeFilter !== 'all' || communityFilter !== 'all' || relationTypeFilter !== 'all' || factTagFilter !== 'all' || mostConnectedOnly) && (
            <button
              type="button"
              onClick={() => {
                setSearchQuery('');
                setNodeTypeFilter('all');
                setCommunityFilter('all');
                setRelationTypeFilter('all');
                setMostConnectedOnly(false);
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
      <div className="flex flex-col gap-3 border-b border-border/40 pb-3 lg:flex-row lg:items-center lg:justify-between">
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

        <div className="flex flex-wrap items-center gap-2">
          <div className="flex items-center gap-1 rounded-xl border border-border/60 bg-card/50 p-1">
            {([
              ['overview', 'Overview'],
              ['all', 'All nodes'],
              ['neighborhood', 'Neighborhood'],
            ] as const).map(([value, label]) => (
              <button
                key={value}
                type="button"
                onClick={() => setGraphMode(value)}
                disabled={value === 'neighborhood' && !selectedNode}
                className={cn(
                  'min-h-9 rounded-lg px-3 text-[11px] font-semibold transition',
                  graphMode === value ? 'bg-amber-500/15 text-amber-400' : 'text-muted-foreground hover:bg-muted hover:text-foreground',
                  value === 'neighborhood' && !selectedNode && 'cursor-not-allowed opacity-40',
                )}
              >
                {label}
              </button>
            ))}
          </div>
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
            <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_300px]">
              <ObsidianGraph
                nodes={graphNodes}
                edges={graphEdges}
                facts={filteredFacts}
                mode={graphMode}
                searchQuery={searchQuery}
                selectedNodeName={selectedNodeName}
                selectedEdgeKey={selectedEdgeKey}
                onSelectNode={handleSelectNode}
                onSelectEdge={handleSelectEdge}
                onModeChange={setGraphMode}
                onVisibleStatsChange={setVisibleGraphStats}
                height={560}
              />
              <MemoryEntityInspector
                node={selectedNode}
                edges={selectedNodeEdges}
                neighbors={selectedNeighbors}
                edgeSemantics={graphSemantics.edgeMetrics}
                selectedEdgeKey={selectedEdgeKey}
                onSelectEdge={handleSelectEdge}
                onFocusNeighborhood={() => setGraphMode('neighborhood')}
                onClear={() => {
                  setSelectedNodeName(null);
                  setSelectedEdgeKey(null);
                }}
              />
            </div>
            {selectedEdge && (
              <div className="flex flex-wrap items-center gap-x-3 gap-y-1 rounded-xl border border-cyan-400/20 bg-cyan-400/5 px-3 py-2 text-[10px] text-muted-foreground">
                <span className="font-semibold text-cyan-300">Selected relation</span>
                <span className="font-mono text-foreground">{selectedEdge.from_name} → {selectedEdge.to_name}</span>
                <span className="rounded bg-cyan-400/10 px-1.5 py-0.5 font-mono text-cyan-300">{selectedEdge.relation_type}</span>
                <span>Confidence: {formatConfidence(graphSemantics.edgeMetrics.get(selectedEdgeKey || '')?.confidence ?? null)}</span>
                <span>Provenance: {formatProvenance(graphSemantics.edgeMetrics.get(selectedEdgeKey || '')?.provenance ?? 'not_recorded')}</span>
              </div>
            )}
            <div className="flex items-center justify-between text-[11px] text-muted-foreground px-1 select-none">
              <span>Overview groups records for readability. Search, zoom, or select a cluster to reveal individual entities.</span>
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
                    OpenZ utilizes an asynchronous <strong>Self-Improvement Memory Curator</strong> that runs in the background after turns. It compiles conversation observations into long-term facts in the configured runtime database (<code>{memoryDbPath}</code>).
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
                filteredFacts.map((fact) => (
                  <div
                    key={`${fact.timestamp}-${fact.text}`}
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
