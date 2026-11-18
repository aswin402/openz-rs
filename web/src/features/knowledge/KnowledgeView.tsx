import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  AlertCircle,
  ArrowLeft,
  BrainCircuit,
  Check,
  ChevronDown,
  ChevronUp,
  Copy,
  Database,
  Download,
  ExternalLink,
  FileCode,
  Focus,
  KeyRound,
  Layers,
  Maximize2,
  RefreshCw,
  RotateCcw,
  Search,
  SlidersHorizontal,
  Sparkles,
  X,
  ZoomIn,
  ZoomOut,
} from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Switch } from '../../components/ui/switch';
import { TIMING_MS } from '../../config/runtime';
import { cn } from '../../lib/utils';
import { wsService } from '../../services/websocket';
import { formatObservations } from '../../shared/lib/format';
import { useOpenZStore } from '../../store/useOpenZStore';
import type { CognitiveEdge, CognitiveNode } from '../../types/openz';
import {
  DEFAULT_GRAPH_DISPLAY,
  ObsidianGraph,
  type ObsidianGraphHandle,
} from './ObsidianGraph';
import {
  GRAPH_CATEGORY_CATALOG,
  buildCoreUniverseGraph,
  getCategoryDescriptor,
  normalizeGraphCategory,
  type CanonicalGraphCategory,
} from './coreGraphBuilder';
import type { GraphMode } from './graphLayout';
import {
  buildGraphSemantics,
  edgeKey,
  formatConfidence,
  formatProvenance,
  type EdgeSemantics,
  type NodeMetrics,
} from './graphSemantics';
import { getNodeColor, type GraphDisplayState } from './graph/graphCanvas';

export const GraphVisualizer: React.FC<{ nodes: CognitiveNode[]; edges: CognitiveEdge[] }> = ({
  nodes,
  edges,
}) => {
  const facts = useOpenZStore((s) => s.cognitiveStats.facts || []);
  return <ObsidianGraph nodes={nodes} edges={edges} facts={facts} height={500} />;
};

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

type ControlSection = 'explorer' | 'display' | 'vault';
type VaultDrawerMode = 'none' | 'facts' | 'markdown';
type GraphDataScope = 'universe' | 'graph-db';

const DISPLAY_OPTIONS: ReadonlyArray<{
  key: keyof GraphDisplayState;
  label: string;
  hint: string;
}> = [
  { key: 'showLabels', label: 'Star labels', hint: 'Entity titles on hubs & zoom' },
  { key: 'showGlow', label: 'Nebula & corona', hint: 'Category aura & hub glow' },
  { key: 'curvedLinks', label: 'Curved filaments', hint: 'Smooth arc trajectories' },
  { key: 'showParticles', label: 'Energy flow', hint: 'Directional link particles' },
  { key: 'showSpaceField', label: 'Deep starfield', hint: 'Parallax cosmic backdrop' },
  { key: 'showOrbits', label: 'Neighbor orbits', hint: '1-hop orbital ring on focus' },
  { key: 'showGrid', label: 'Coordinate grid', hint: 'Spatial reference grid' },
];

interface EntityInspectorProps {
  node: CognitiveNode | null;
  nodeMetric: NodeMetrics | undefined;
  edges: CognitiveEdge[];
  neighbors: CognitiveNode[];
  edgeSemantics: Map<string, EdgeSemantics>;
  selectedEdge: CognitiveEdge | null;
  selectedEdgeKey: string | null;
  graphMode: GraphMode;
  onSelectNode: (nodeName: string) => void;
  onSelectEdge: (key: string) => void;
  onToggleNeighborhood: () => void;
  onCenterNode: (nodeName: string) => void;
  onClear: () => void;
}

const FloatingEntityInspector: React.FC<EntityInspectorProps> = ({
  node,
  nodeMetric,
  edges,
  neighbors,
  edgeSemantics,
  selectedEdge,
  selectedEdgeKey,
  graphMode,
  onSelectNode,
  onSelectEdge,
  onToggleNeighborhood,
  onCenterNode,
  onClear,
}) => {
  if (!node && !selectedEdge) return null;

  const descriptor = node ? getCategoryDescriptor(node.entity_type) : undefined;
  const entityColor = node ? getNodeColor(node.entity_type).main : undefined;
  const importancePct = Math.round((nodeMetric?.importance ?? 0) * 100);
  const selectedEdgeSemantics = selectedEdgeKey ? edgeSemantics.get(selectedEdgeKey) : undefined;

  return (
    <aside
      aria-label="Selected entity inspector"
      className="pointer-events-auto flex max-h-[calc(100vh-5.5rem)] w-full flex-col overflow-hidden rounded-2xl border border-border/70 bg-card/86 shadow-2xl ring-1 ring-white/[0.05] backdrop-blur-2xl"
    >
      <div className="flex items-start justify-between gap-2 border-b border-border/55 px-3.5 py-2.5">
        <div className="min-w-0 flex-1">
          {node && descriptor ? (
            <>
              <div className="flex items-center gap-1.5 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
                <span
                  className="h-2.5 w-2.5 shrink-0 rounded-full shadow-xs"
                  style={{ backgroundColor: entityColor }}
                />
                <span className="truncate">{descriptor.label}</span>
              </div>
              <h2 className="mt-0.5 break-words text-sm font-bold leading-snug text-foreground">
                {node.name}
              </h2>
            </>
          ) : (
            <>
              <div className="text-[10px] font-semibold uppercase tracking-wider text-info">
                Selected Relation
              </div>
              <h2 className="mt-0.5 break-words text-xs font-bold text-foreground">
                {selectedEdge?.from_name} → {selectedEdge?.to_name}
              </h2>
            </>
          )}
        </div>

        <button
          type="button"
          onClick={onClear}
          className="rounded-lg p-1.5 text-muted-foreground transition hover:bg-muted hover:text-foreground"
          aria-label="Close inspector"
        >
          <X className="h-3.5 w-3.5" />
        </button>
      </div>

      <div className="min-h-0 flex-1 space-y-3 overflow-y-auto p-3.5 scrollbar-thin">
        {selectedEdge && (
          <div className="rounded-xl border border-info/30 bg-info/10 p-2.5 text-[10px]">
            <div className="flex items-center justify-between gap-2">
              <span className="font-semibold text-info">Active Relation</span>
              <span className="rounded bg-info/15 px-1.5 py-0.5 font-mono text-info">
                {selectedEdge.relation_type}
              </span>
            </div>
            <div className="mt-1 font-mono text-[10px] text-foreground">
              {selectedEdge.from_name} → {selectedEdge.to_name}
            </div>
            <div className="mt-1 flex flex-wrap gap-x-2 gap-y-0.5 font-mono text-[9px] text-muted-foreground">
              <span>Confidence: {formatConfidence(selectedEdgeSemantics?.confidence ?? null)}</span>
              <span>
                Source:{' '}
                {selectedEdge.source ||
                  formatProvenance(selectedEdgeSemantics?.provenance ?? 'not_recorded')}
              </span>
            </div>
          </div>
        )}

        {node && (
          <>
            <div className="flex items-center gap-1.5">
              <button
                type="button"
                onClick={onToggleNeighborhood}
                className={cn(
                  'flex-1 rounded-xl border px-2.5 py-1.5 text-[11px] font-semibold transition',
                  graphMode === 'neighborhood'
                    ? 'border-primary/45 bg-primary/16 text-primary'
                    : 'border-border/65 bg-muted/25 text-foreground hover:border-primary/35 hover:bg-primary/10',
                )}
              >
                {graphMode === 'neighborhood' ? 'Show Full Universe' : 'Isolate 1-Hop Orbit'}
              </button>
              <button
                type="button"
                onClick={() => onCenterNode(node.name)}
                className="inline-flex items-center gap-1 rounded-xl border border-border/65 bg-muted/25 px-2.5 py-1.5 text-[11px] font-semibold text-foreground transition hover:bg-muted"
                title="Center camera on entity"
              >
                <Focus className="h-3 w-3 text-primary" />
                Center
              </button>
            </div>

            <div className="grid grid-cols-3 gap-1.5 text-center">
              <div className="rounded-xl border border-border/50 bg-background/50 px-2 py-1.5">
                <div className="text-[9px] uppercase tracking-wider text-muted-foreground">
                  Links
                </div>
                <div className="mt-0.5 font-mono text-sm font-bold text-foreground">
                  {edges.length}
                </div>
              </div>
              <div className="rounded-xl border border-border/50 bg-background/50 px-2 py-1.5">
                <div className="text-[9px] uppercase tracking-wider text-muted-foreground">
                  Neighbors
                </div>
                <div className="mt-0.5 font-mono text-sm font-bold text-foreground">
                  {neighbors.length}
                </div>
              </div>
              <div className="rounded-xl border border-border/50 bg-background/50 px-2 py-1.5">
                <div className="text-[9px] uppercase tracking-wider text-muted-foreground">
                  Centrality
                </div>
                <div className="mt-0.5 font-mono text-sm font-bold text-foreground">
                  {importancePct}%
                </div>
              </div>
            </div>

            <div className="rounded-xl border border-border/55 bg-background/55 p-2.5">
              <div className="text-[9px] font-semibold uppercase tracking-wider text-muted-foreground">
                Details &amp; Observations
              </div>
              <p className="mt-1.5 max-h-36 overflow-y-auto whitespace-pre-wrap break-words text-[11px] leading-relaxed text-foreground/88 scrollbar-thin">
                {formatObservations(node.observations)}
              </p>
            </div>

            {neighbors.length > 0 && (
              <div>
                <div className="mb-1.5 text-[9px] font-semibold uppercase tracking-wider text-muted-foreground">
                  Connected Stars ({neighbors.length})
                </div>
                <div className="flex max-h-28 flex-wrap gap-1 overflow-y-auto scrollbar-thin">
                  {neighbors.map((neighbor) => (
                    <button
                      key={neighbor.name}
                      type="button"
                      onClick={() => onSelectNode(neighbor.name)}
                      className="inline-flex max-w-full items-center gap-1.5 rounded-lg border border-border/55 bg-background/50 px-2 py-1 text-[10px] font-medium text-foreground transition hover:border-primary/40 hover:bg-primary/10"
                    >
                      <span
                        className="h-2 w-2 shrink-0 rounded-full"
                        style={{ backgroundColor: getNodeColor(neighbor.entity_type).main }}
                      />
                      <span className="truncate">{neighbor.name}</span>
                    </button>
                  ))}
                </div>
              </div>
            )}

            {edges.length > 0 && (
              <div>
                <div className="mb-1.5 text-[9px] font-semibold uppercase tracking-wider text-muted-foreground">
                  Relationships ({edges.length})
                </div>
                <div className="space-y-1">
                  {edges.slice(0, 24).map((edge) => {
                    const relationKey = edgeKey(edge);
                    const isSelected = selectedEdgeKey === relationKey;
                    return (
                      <button
                        key={relationKey}
                        type="button"
                        onClick={() => onSelectEdge(relationKey)}
                        className={cn(
                          'w-full rounded-xl border px-2.5 py-1.5 text-left text-[10px] transition',
                          isSelected
                            ? 'border-info/45 bg-info/12'
                            : 'border-border/50 bg-background/45 hover:border-primary/30 hover:bg-muted/35',
                        )}
                      >
                        <div className="flex items-center justify-between gap-2">
                          <span className="min-w-0 truncate font-medium text-foreground">
                            {edge.from_name} → {edge.to_name}
                          </span>
                          <span className="shrink-0 rounded bg-primary/12 px-1.5 py-0.5 font-mono text-[9px] text-primary">
                            {edge.relation_type}
                          </span>
                        </div>
                      </button>
                    );
                  })}
                </div>
              </div>
            )}
          </>
        )}
      </div>
    </aside>
  );
};

export const KnowledgeView: React.FC = () => {
  const cognitiveStats = useOpenZStore((s) => s.cognitiveStats);
  const setIsMemoryOpen = useOpenZStore((s) => s.setIsMemoryOpen);
  const setActiveView = useOpenZStore((s) => s.setActiveView);
  const runtimeInventory = useOpenZStore((s) => s.runtimeInventory);
  const isActivityPanelOpen = useOpenZStore((s) => s.isActivityPanelOpen);

  const graphControlsRef = useRef<ObsidianGraphHandle | null>(null);

  const [controlCollapsed, setControlCollapsed] = useState(false);
  const [controlSection, setControlSection] = useState<ControlSection>('explorer');
  const [vaultDrawer, setVaultDrawer] = useState<VaultDrawerMode>('none');
  const [dataScope, setDataScope] = useState<GraphDataScope>('universe');
  const [includeLeafTools, setIncludeLeafTools] = useState(false);

  const [copied, setCopied] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [categoryFilter, setCategoryFilter] = useState<'all' | CanonicalGraphCategory>('all');
  const [mostConnectedOnly, setMostConnectedOnly] = useState(false);
  const [factTagFilter, setFactTagFilter] = useState('all');
  const [sortMode, setSortMode] = useState<'importance' | 'newest'>('importance');
  const [graphMode, setGraphMode] = useState<GraphMode>('all');
  const [selectedNodeName, setSelectedNodeName] = useState<string | null>(null);
  const [selectedEdgeKey, setSelectedEdgeKey] = useState<string | null>(null);

  const [display, setDisplay] = useState<GraphDisplayState>(() => ({
    ...DEFAULT_GRAPH_DISPLAY,
    curvedLinks: true,
  }));

  useEffect(() => {
    wsService.requestCognitiveMemory();
    wsService.requestRuntimeInventory();

    const interval = setInterval(() => {
      wsService.requestCognitiveMemory();
    }, TIMING_MS.memoryPoll);

    return () => clearInterval(interval);
  }, []);

  const handleRefresh = useCallback(() => {
    setIsRefreshing(true);
    wsService.requestCognitiveMemory();
    wsService.requestRuntimeInventory();
    setTimeout(() => setIsRefreshing(false), 650);
  }, []);

  const handleDisplayChange = useCallback((key: keyof GraphDisplayState, value: boolean) => {
    setDisplay((current) => ({ ...current, [key]: value }));
  }, []);

  const coreUniverse = useMemo(
    () =>
      buildCoreUniverseGraph(
        cognitiveStats.nodes ?? [],
        cognitiveStats.edges ?? [],
        cognitiveStats.facts ?? [],
        runtimeInventory,
        {
          includeCoreArchitecture: dataScope === 'universe',
          includeLeafTools,
        },
      ),
    [
      cognitiveStats.nodes,
      cognitiveStats.edges,
      cognitiveStats.facts,
      runtimeInventory,
      dataScope,
      includeLeafTools,
    ],
  );

  const allNodes = coreUniverse.nodes;
  const allEdges = coreUniverse.edges;
  const cleanFacts = coreUniverse.cleanFacts;

  const graphSemantics = useMemo(
    () => buildGraphSemantics(allNodes, allEdges),
    [allEdges, allNodes],
  );

  const categoryCounts = useMemo(() => {
    const counts = new Map<CanonicalGraphCategory, number>();
    for (const node of allNodes) {
      const cat = normalizeGraphCategory(node.entity_type);
      counts.set(cat, (counts.get(cat) ?? 0) + 1);
    }
    return counts;
  }, [allNodes]);

  const activeCategories = useMemo(
    () =>
      Object.values(GRAPH_CATEGORY_CATALOG)
        .filter((cat) => (categoryCounts.get(cat.id) ?? 0) > 0)
        .sort((a, b) => a.order - b.order),
    [categoryCounts],
  );

  const degreeThreshold = useMemo(() => {
    const degrees = Array.from(graphSemantics.nodeMetrics.values())
      .map((metric) => metric.degree)
      .sort((left, right) => right - left);
    return degrees.length === 0
      ? 0
      : degrees[Math.min(degrees.length - 1, Math.floor(degrees.length * 0.25))];
  }, [graphSemantics]);

  const topHubNode = useMemo<CognitiveNode | null>(() => {
    return allNodes.reduce<CognitiveNode | null>((best, node) => {
      const deg = graphSemantics.nodeMetrics.get(node.name)?.degree ?? 0;
      const bestDeg = best ? (graphSemantics.nodeMetrics.get(best.name)?.degree ?? 0) : -1;
      return deg > bestDeg ? node : best;
    }, null);
  }, [allNodes, graphSemantics]);

  const normalizedSearch = searchQuery.trim().toLowerCase();
  const matchingEdgeEndpoints = useMemo(() => {
    if (!normalizedSearch) return new Set<string>();
    return new Set(
      allEdges
        .filter((edge) =>
          `${edge.from_name} ${edge.to_name} ${edge.relation_type}`
            .toLowerCase()
            .includes(normalizedSearch),
        )
        .flatMap((edge) => [edge.from_name, edge.to_name]),
    );
  }, [allEdges, normalizedSearch]);

  const filteredNodes = useMemo(() => {
    const directMatches = new Set<string>();
    for (const node of allNodes) {
      const metric = graphSemantics.nodeMetrics.get(node.name);
      const cat = normalizeGraphCategory(node.entity_type);
      const matchesCat = categoryFilter === 'all' || cat === categoryFilter;
      const matchesHub = !mostConnectedOnly || (metric?.degree ?? 0) >= degreeThreshold;
      const haystack = `${node.name} ${cat} ${formatObservations(node.observations)}`.toLowerCase();
      const matchesQuery =
        !normalizedSearch ||
        haystack.includes(normalizedSearch) ||
        matchingEdgeEndpoints.has(node.name);
      if (matchesCat && matchesHub && matchesQuery) {
        directMatches.add(node.name);
      }
    }

    // When filtering by a category, keep its 1-hop anchor hubs visible so stars remain connected!
    if (categoryFilter !== 'all' && directMatches.size > 0) {
      for (const edge of allEdges) {
        if (directMatches.has(edge.from_name) && edge.to_name === 'OpenZ') {
          directMatches.add(edge.to_name);
        }
        if (directMatches.has(edge.to_name) && edge.from_name === 'OpenZ') {
          directMatches.add(edge.from_name);
        }
      }
    }

    return allNodes.filter((n) => directMatches.has(n.name));
  }, [
    allNodes,
    allEdges,
    categoryFilter,
    mostConnectedOnly,
    degreeThreshold,
    graphSemantics.nodeMetrics,
    normalizedSearch,
    matchingEdgeEndpoints,
  ]);

  const filteredNodeNames = useMemo(
    () => new Set(filteredNodes.map((n) => n.name)),
    [filteredNodes],
  );

  const filteredEdges = useMemo(
    () =>
      allEdges.filter(
        (edge) => filteredNodeNames.has(edge.from_name) && filteredNodeNames.has(edge.to_name),
      ),
    [allEdges, filteredNodeNames],
  );

  const sortedExplorerNodes = useMemo(() => {
    return [...filteredNodes].sort((a, b) => {
      const degA = graphSemantics.nodeMetrics.get(a.name)?.degree ?? 0;
      const degB = graphSemantics.nodeMetrics.get(b.name)?.degree ?? 0;
      return degB - degA || a.name.localeCompare(b.name);
    });
  }, [filteredNodes, graphSemantics.nodeMetrics]);

  const factTags = useMemo(
    () =>
      Array.from(
        new Set(
          cleanFacts.flatMap((fact) =>
            fact.tags
              .replace('[', '')
              .replace(']', '')
              .replaceAll('"', '')
              .split(',')
              .map((tag) => tag.trim())
              .filter(Boolean),
          ),
        ),
      ).sort(),
    [cleanFacts],
  );

  const filteredFacts = useMemo(
    () =>
      cleanFacts
        .filter((fact) => {
          const matchesTag =
            factTagFilter === 'all' ||
            fact.tags.toLowerCase().includes(factTagFilter.toLowerCase());
          const haystack = `${fact.text} ${fact.tags} ${fact.timestamp}`.toLowerCase();
          return matchesTag && (!normalizedSearch || haystack.includes(normalizedSearch));
        })
        .sort((a, b) =>
          sortMode === 'importance'
            ? b.importance - a.importance
            : new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime(),
        ),
    [cleanFacts, factTagFilter, normalizedSearch, sortMode],
  );

  const selectedNode = useMemo(
    () =>
      selectedNodeName ? allNodes.find((node) => node.name === selectedNodeName) || null : null,
    [allNodes, selectedNodeName],
  );

  const selectedNodeEdges = useMemo(
    () =>
      selectedNode
        ? allEdges.filter(
            (edge) => edge.from_name === selectedNode.name || edge.to_name === selectedNode.name,
          )
        : [],
    [allEdges, selectedNode],
  );

  const selectedNeighbors = useMemo(() => {
    if (!selectedNode) return [];
    const neighborNames = new Set(
      selectedNodeEdges
        .flatMap((edge) => [edge.from_name, edge.to_name])
        .filter((name) => name !== selectedNode.name),
    );
    return allNodes.filter((node) => neighborNames.has(node.name));
  }, [allNodes, selectedNode, selectedNodeEdges]);

  const graphNodes = useMemo(
    () =>
      selectedNode && !filteredNodes.some((node) => node.name === selectedNode.name)
        ? [...filteredNodes, selectedNode]
        : filteredNodes,
    [filteredNodes, selectedNode],
  );

  const graphEdges = useMemo(() => {
    const names = new Set(graphNodes.map((node) => node.name));
    return filteredEdges.filter((edge) => names.has(edge.from_name) && names.has(edge.to_name));
  }, [filteredEdges, graphNodes]);

  const selectedEdge = useMemo(
    () =>
      selectedEdgeKey ? allEdges.find((edge) => edgeKey(edge) === selectedEdgeKey) || null : null,
    [allEdges, selectedEdgeKey],
  );

  const handleSelectNode = useCallback((nodeName: string | null) => {
    setSelectedNodeName(nodeName);
    setSelectedEdgeKey(null);
  }, []);

  const focusNodeOnCanvas = useCallback(
    (nodeName: string) => {
      handleSelectNode(nodeName);
      requestAnimationFrame(() => {
        graphControlsRef.current?.focusNodeByName(nodeName);
      });
    },
    [handleSelectNode],
  );

  const handleSelectEdge = useCallback(
    (key: string | null) => {
      setSelectedEdgeKey(key);
      if (!key) return;
      const edge = allEdges.find((candidate) => edgeKey(candidate) === key);
      if (edge) {
        setSelectedNodeName(edge.from_name);
      }
    },
    [allEdges],
  );

  const memoryDbPath =
    cognitiveStats.paths?.memoryDb ||
    runtimeInventory?.paths.memoryDb ||
    '~/.openz/memory.db';
  const graphDbPath =
    cognitiveStats.paths?.graphDb ||
    runtimeInventory?.paths.graphDb ||
    '~/.openz/graph_memory.db';

  const generateMarkdownString = useCallback(() => {
    let md = `# OpenZ Core Knowledge Graph Snapshot\n\n`;
    md += `- **Generated**: \`${new Date().toLocaleString()}\`\n`;
    md += `- **Scope**: \`${dataScope}\`\n`;
    md += `- **Memory DB**: \`${memoryDbPath}\`\n`;
    md += `- **Graph DB**: \`${graphDbPath}\`\n\n`;

    md += `## 1. Entities (${graphNodes.length})\n\n`;
    for (const n of graphNodes) {
      md += `### [[${n.name}]] (${n.entity_type})\n`;
      md += `- ${formatObservations(n.observations)}\n\n`;
    }

    md += `## 2. Relationships (${graphEdges.length})\n\n`;
    if (graphEdges.length > 0) {
      md += `| Source | Target | Relation |\n| :--- | :--- | :--- |\n`;
      for (const e of graphEdges) {
        md += `| [[${e.from_name}]] | [[${e.to_name}]] | \`${e.relation_type}\` |\n`;
      }
      md += `\n`;
    }

    md += `## 3. Curated Cognitive Facts (${filteredFacts.length})\n\n`;
    for (const f of filteredFacts) {
      md += `- ${f.text} *(importance: ${f.importance.toFixed(2)})*\n`;
    }
    return md;
  }, [dataScope, filteredFacts, graphDbPath, graphEdges, graphNodes, memoryDbPath]);

  const handleCopyMarkdown = useCallback(() => {
    navigator.clipboard.writeText(generateMarkdownString());
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }, [generateMarkdownString]);

  const handleDownloadMarkdown = useCallback(() => {
    downloadTextFile(
      'openz-core-graph.md',
      generateMarkdownString(),
      'text/markdown;charset=utf-8',
    );
  }, [generateMarkdownString]);

  const handleDownloadJson = useCallback(() => {
    downloadTextFile(
      'openz-core-graph.json',
      JSON.stringify(
        {
          generatedAt: new Date().toISOString(),
          scope: dataScope,
          nodes: graphNodes,
          edges: graphEdges,
          facts: filteredFacts,
          workingMemoryKeys: cognitiveStats.workingMemoryKeys,
        },
        null,
        2,
      ),
      'application/json;charset=utf-8',
    );
  }, [cognitiveStats.workingMemoryKeys, dataScope, filteredFacts, graphEdges, graphNodes]);

  const rightOffsetClass = isActivityPanelOpen ? 'right-3' : 'right-14 md:right-16';

  return (
    <div className="relative h-full w-full overflow-hidden bg-background select-none">
      {/* Full-Viewport Obsidian + Graphify Universe Canvas */}
      <ObsidianGraph
        nodes={graphNodes}
        edges={graphEdges}
        facts={filteredFacts}
        mode={graphMode}
        searchQuery={searchQuery}
        selectedNodeName={selectedNodeName}
        selectedEdgeKey={selectedEdgeKey}
        display={display}
        onDisplayChange={handleDisplayChange}
        onSelectNode={handleSelectNode}
        onSelectEdge={handleSelectEdge}
        onModeChange={setGraphMode}
        controlsRef={graphControlsRef}
        fullBleed
        showBuiltInChrome={false}
        height="100%"
      />

      {/* Top-Left Compact Obsidian Control Pod */}
      <section
        aria-label="Universe graph controls"
        className="pointer-events-auto absolute left-14 top-3 z-20 flex max-h-[calc(100vh-5.25rem)] w-[min(324px,calc(100vw-5.25rem))] flex-col overflow-hidden rounded-2xl border border-border/70 bg-card/84 shadow-2xl ring-1 ring-white/[0.05] backdrop-blur-2xl md:left-3"
      >
        <div className="flex items-center gap-2 border-b border-border/55 px-3 py-2.5">
          <button
            type="button"
            onClick={() => setActiveView('chats')}
            className="rounded-lg border border-border/60 bg-background/60 p-1.5 text-muted-foreground transition hover:bg-muted hover:text-foreground"
            title="Back to chat"
            aria-label="Back to chat"
          >
            <ArrowLeft className="h-3.5 w-3.5" />
          </button>

          <div className="flex min-w-0 flex-1 items-center gap-1.5">
            <BrainCircuit className="h-4 w-4 shrink-0 text-primary" />
            <h1 className="truncate text-xs font-bold tracking-tight text-foreground">
              Cognitive Universe
            </h1>
          </div>

          <span className="rounded-full bg-foreground/[0.06] px-2 py-0.5 font-mono text-[10px] text-muted-foreground">
            {graphNodes.length} · {graphEdges.length}e
          </span>

          <button
            type="button"
            onClick={handleRefresh}
            className="rounded-lg p-1.5 text-muted-foreground transition hover:bg-muted hover:text-foreground"
            title="Sync core graph"
            aria-label="Sync core graph"
          >
            <RefreshCw className={cn('h-3.5 w-3.5', isRefreshing && 'animate-spin text-primary')} />
          </button>

          <button
            type="button"
            onClick={() => setControlCollapsed((prev) => !prev)}
            className="rounded-lg p-1.5 text-muted-foreground transition hover:bg-muted hover:text-foreground"
            aria-label={controlCollapsed ? 'Expand controls' : 'Collapse controls'}
          >
            {controlCollapsed ? (
              <ChevronDown className="h-3.5 w-3.5" />
            ) : (
              <ChevronUp className="h-3.5 w-3.5" />
            )}
          </button>
        </div>

        {!controlCollapsed && (
          <div className="min-h-0 flex-1 space-y-2.5 overflow-y-auto p-3 scrollbar-thin">
            {/* Search */}
            <div className="relative">
              <Search className="pointer-events-none absolute left-2.5 top-2 h-3.5 w-3.5 text-muted-foreground" />
              <input
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder="Search stars, skills, agents, facts…"
                aria-label="Search cognitive universe"
                className="h-8 w-full rounded-xl border border-border/65 bg-background/75 pl-8 pr-7 text-xs text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-primary/50"
              />
              {searchQuery && (
                <button
                  type="button"
                  onClick={() => setSearchQuery('')}
                  className="absolute right-2 top-1.5 rounded p-0.5 text-muted-foreground hover:text-foreground"
                  aria-label="Clear search query"
                >
                  <X className="h-3.5 w-3.5" />
                </button>
              )}
            </div>

            {/* Data Source Scope: Core Universe vs Graph DB Only */}
            <div className="grid grid-cols-2 gap-1 rounded-xl border border-border/55 bg-background/50 p-1 text-[10px]">
              <button
                type="button"
                onClick={() => setDataScope('universe')}
                className={cn(
                  'rounded-lg px-2 py-1 font-semibold transition',
                  dataScope === 'universe'
                    ? 'bg-primary text-primary-foreground shadow-2xs'
                    : 'text-muted-foreground hover:text-foreground',
                )}
              >
                Core Universe
              </button>
              <button
                type="button"
                onClick={() => setDataScope('graph-db')}
                className={cn(
                  'rounded-lg px-2 py-1 font-semibold transition',
                  dataScope === 'graph-db'
                    ? 'bg-primary text-primary-foreground shadow-2xs'
                    : 'text-muted-foreground hover:text-foreground',
                )}
              >
                Memory DB Only
              </button>
            </div>

            {/* Graph Lens */}
            <div className="grid grid-cols-3 gap-1 rounded-xl border border-border/55 bg-muted/20 p-1 text-[10px]">
              {(
                [
                  { id: 'all', label: 'Universe' },
                  { id: 'overview', label: 'Nebulae' },
                  { id: 'neighborhood', label: '1-Hop Orbit' },
                ] as const
              ).map((lens) => (
                <button
                  key={lens.id}
                  type="button"
                  onClick={() => {
                    if (lens.id === 'neighborhood' && !selectedNodeName && topHubNode) {
                      setSelectedNodeName(topHubNode.name);
                    }
                    setGraphMode(lens.id);
                  }}
                  className={cn(
                    'rounded-lg px-2 py-1 font-semibold transition',
                    graphMode === lens.id
                      ? 'bg-foreground/12 text-foreground shadow-2xs'
                      : 'text-muted-foreground hover:text-foreground',
                  )}
                >
                  {lens.label}
                </button>
              ))}
            </div>

            {/* Sub-Tabs */}
            <div className="flex items-center gap-1 border-b border-border/50 pb-2 text-[10px]">
              {(
                [
                  { id: 'explorer', label: 'Stars', icon: Layers },
                  { id: 'display', label: 'Visuals', icon: SlidersHorizontal },
                  { id: 'vault', label: 'Vault', icon: Database },
                ] as const
              ).map((tab) => {
                const Icon = tab.icon;
                return (
                  <button
                    key={tab.id}
                    type="button"
                    onClick={() => setControlSection(tab.id)}
                    className={cn(
                      'inline-flex items-center gap-1 rounded-lg px-2.5 py-1 font-semibold transition',
                      controlSection === tab.id
                        ? 'bg-primary/14 text-primary'
                        : 'text-muted-foreground hover:bg-muted/50 hover:text-foreground',
                    )}
                  >
                    <Icon className="h-3 w-3" />
                    <span>{tab.label}</span>
                  </button>
                );
              })}

              {(categoryFilter !== 'all' || mostConnectedOnly || Boolean(searchQuery)) && (
                <button
                  type="button"
                  onClick={() => {
                    setCategoryFilter('all');
                    setMostConnectedOnly(false);
                    setSearchQuery('');
                  }}
                  className="ml-auto rounded-lg px-2 py-1 font-semibold text-primary hover:underline"
                >
                  Reset
                </button>
              )}
            </div>

            {controlSection === 'explorer' && (
              <div className="space-y-2">
                <div className="flex flex-wrap items-center gap-1.5 text-[10px]">
                  <button
                    type="button"
                    onClick={() => setMostConnectedOnly((prev) => !prev)}
                    className={cn(
                      'inline-flex items-center gap-1 rounded-lg border px-2 py-1 font-semibold transition',
                      mostConnectedOnly
                        ? 'border-primary/40 bg-primary/15 text-primary'
                        : 'border-border/55 bg-background/50 text-muted-foreground hover:text-foreground',
                    )}
                  >
                    <Sparkles className="h-3 w-3" />
                    <span>God Nodes Only</span>
                  </button>

                  {dataScope === 'universe' && runtimeInventory && (
                    <button
                      type="button"
                      onClick={() => setIncludeLeafTools((prev) => !prev)}
                      className={cn(
                        'inline-flex items-center gap-1 rounded-lg border px-2 py-1 font-semibold transition',
                        includeLeafTools
                          ? 'border-primary/40 bg-primary/15 text-primary'
                          : 'border-border/55 bg-background/50 text-muted-foreground hover:text-foreground',
                      )}
                    >
                      <span>+ {runtimeInventory.counts.tools} Tools</span>
                    </button>
                  )}
                </div>

                <div className="max-h-56 space-y-0.5 overflow-y-auto pr-0.5 scrollbar-thin">
                  {sortedExplorerNodes.slice(0, 120).map((node) => {
                    const isSelected = selectedNodeName === node.name;
                    const degree = graphSemantics.nodeMetrics.get(node.name)?.degree ?? 0;
                    const cat = getCategoryDescriptor(node.entity_type);
                    const dotColor = getNodeColor(node.entity_type).main;
                    return (
                      <button
                        key={node.name}
                        type="button"
                        onClick={() => focusNodeOnCanvas(node.name)}
                        className={cn(
                          'flex w-full items-center gap-2 rounded-xl px-2 py-1.5 text-left text-[11px] transition',
                          isSelected
                            ? 'bg-primary/14 text-foreground ring-1 ring-primary/35'
                            : 'text-foreground/85 hover:bg-muted/40',
                        )}
                      >
                        <span
                          className="h-2.5 w-2.5 shrink-0 rounded-full"
                          style={{ backgroundColor: dotColor }}
                        />
                        <span className="min-w-0 flex-1 truncate font-medium">{node.name}</span>
                        <span className="shrink-0 rounded bg-foreground/[0.05] px-1.5 py-0.5 font-mono text-[9px] text-muted-foreground">
                          {cat.shortLabel} · {degree}
                        </span>
                      </button>
                    );
                  })}
                </div>
              </div>
            )}

            {controlSection === 'display' && (
              <div className="space-y-1.5">
                {DISPLAY_OPTIONS.map((item) => (
                  <label
                    key={item.key}
                    className="flex cursor-pointer items-center justify-between gap-2 rounded-xl border border-border/45 bg-background/45 px-2.5 py-1.5"
                  >
                    <span className="min-w-0">
                      <span className="block text-[11px] font-medium text-foreground">
                        {item.label}
                      </span>
                      <span className="block truncate text-[9px] text-muted-foreground">
                        {item.hint}
                      </span>
                    </span>
                    <Switch
                      checked={display[item.key]}
                      onCheckedChange={(checked) => handleDisplayChange(item.key, checked)}
                      aria-label={item.label}
                    />
                  </label>
                ))}
              </div>
            )}

            {controlSection === 'vault' && (
              <div className="space-y-2 text-[11px]">
                <div className="grid grid-cols-2 gap-1.5">
                  <button
                    type="button"
                    onClick={() =>
                      setVaultDrawer((curr) => (curr === 'facts' ? 'none' : 'facts'))
                    }
                    className={cn(
                      'flex items-center justify-center gap-1.5 rounded-xl border px-2.5 py-2 text-[11px] font-semibold transition',
                      vaultDrawer === 'facts'
                        ? 'border-primary/40 bg-primary/15 text-primary'
                        : 'border-border/65 bg-background/60 text-foreground hover:bg-muted/50',
                    )}
                  >
                    <Database className="h-3.5 w-3.5 text-primary" />
                    Facts ({filteredFacts.length})
                  </button>
                  <button
                    type="button"
                    onClick={() =>
                      setVaultDrawer((curr) => (curr === 'markdown' ? 'none' : 'markdown'))
                    }
                    className={cn(
                      'flex items-center justify-center gap-1.5 rounded-xl border px-2.5 py-2 text-[11px] font-semibold transition',
                      vaultDrawer === 'markdown'
                        ? 'border-primary/40 bg-primary/15 text-primary'
                        : 'border-border/65 bg-background/60 text-foreground hover:bg-muted/50',
                    )}
                  >
                    <FileCode className="h-3.5 w-3.5 text-primary" />
                    Snapshot .md
                  </button>
                </div>

                <div className="flex items-center gap-1.5">
                  <button
                    type="button"
                    onClick={handleDownloadMarkdown}
                    className="flex flex-1 items-center justify-center gap-1 rounded-xl border border-border/65 bg-muted/25 px-2.5 py-1.5 text-[10px] font-semibold text-muted-foreground transition hover:bg-muted hover:text-foreground"
                  >
                    <Download className="h-3 w-3" /> Export .md
                  </button>
                  <button
                    type="button"
                    onClick={handleDownloadJson}
                    className="flex flex-1 items-center justify-center gap-1 rounded-xl border border-border/65 bg-muted/25 px-2.5 py-1.5 text-[10px] font-semibold text-muted-foreground transition hover:bg-muted hover:text-foreground"
                  >
                    <Download className="h-3 w-3" /> Export .json
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      setIsMemoryOpen(true);
                      wsService.requestCognitiveMemory();
                    }}
                    className="inline-flex items-center justify-center rounded-xl border border-border/65 bg-muted/25 p-1.5 text-muted-foreground transition hover:bg-muted hover:text-foreground"
                    title="Inspect database paths & working memory scope"
                    aria-label="Inspect database paths"
                  >
                    <ExternalLink className="h-3.5 w-3.5" />
                  </button>
                </div>

                {cognitiveStats.workingMemoryKeys.length > 0 && (
                  <div className="rounded-xl border border-border/55 bg-background/45 p-2">
                    <div className="flex items-center gap-1 text-[9px] font-semibold uppercase tracking-wider text-muted-foreground">
                      <KeyRound className="h-3 w-3 text-primary" />
                      Working Scope ({cognitiveStats.workingMemoryKeys.length})
                    </div>
                    <div className="mt-1.5 flex max-h-16 flex-wrap gap-1 overflow-y-auto scrollbar-thin">
                      {cognitiveStats.workingMemoryKeys.map((key) => (
                        <span
                          key={key}
                          className="rounded border border-border/60 bg-muted/35 px-1.5 py-0.5 font-mono text-[9px] text-foreground"
                        >
                          {key}
                        </span>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            )}
          </div>
        )}
      </section>

      {/* Bottom-Center Interactive Category Constellation Legend */}
      {activeCategories.length > 0 && (
        <div className="pointer-events-none absolute bottom-3 left-1/2 z-20 flex max-w-[calc(100vw-15rem)] -translate-x-1/2 items-center">
          <div className="pointer-events-auto flex items-center gap-1 overflow-x-auto rounded-full border border-border/70 bg-card/84 px-2 py-1 shadow-xl ring-1 ring-white/[0.05] backdrop-blur-2xl scrollbar-none">
            <button
              type="button"
              onClick={() => setCategoryFilter('all')}
              className={cn(
                'shrink-0 rounded-full px-2.5 py-1 text-[10.5px] font-semibold transition',
                categoryFilter === 'all'
                  ? 'bg-foreground/12 text-foreground'
                  : 'text-muted-foreground hover:text-foreground',
              )}
            >
              All ({allNodes.length})
            </button>

            {activeCategories.map((cat) => {
              const active = categoryFilter === cat.id;
              const count = categoryCounts.get(cat.id) ?? 0;
              const color = getNodeColor(cat.id).main;
              return (
                <button
                  key={cat.id}
                  type="button"
                  onClick={() => setCategoryFilter(active ? 'all' : cat.id)}
                  className={cn(
                    'inline-flex shrink-0 items-center gap-1.5 rounded-full px-2.5 py-1 text-[10.5px] font-medium transition',
                    active
                      ? 'bg-foreground/14 font-semibold text-foreground ring-1 ring-border'
                      : 'text-muted-foreground hover:bg-foreground/[0.05] hover:text-foreground',
                  )}
                >
                  <span
                    className="h-2 w-2 rounded-full"
                    style={{ backgroundColor: color }}
                  />
                  <span>{cat.shortLabel}</span>
                  <span className="font-mono text-[9.5px] opacity-70">{count}</span>
                </button>
              );
            })}
          </div>
        </div>
      )}

      {/* Top-Right Selected Entity & Relation Glass Inspector */}
      <div
        className={cn(
          'pointer-events-none absolute top-3 z-20 flex w-[min(340px,calc(100vw-4.75rem))] justify-end',
          rightOffsetClass,
        )}
      >
        <FloatingEntityInspector
          node={selectedNode}
          nodeMetric={
            selectedNode ? graphSemantics.nodeMetrics.get(selectedNode.name) : undefined
          }
          edges={selectedNodeEdges}
          neighbors={selectedNeighbors}
          edgeSemantics={graphSemantics.edgeMetrics}
          selectedEdge={selectedEdge}
          selectedEdgeKey={selectedEdgeKey}
          graphMode={graphMode}
          onSelectNode={focusNodeOnCanvas}
          onSelectEdge={handleSelectEdge}
          onToggleNeighborhood={() =>
            setGraphMode((curr) => (curr === 'neighborhood' ? 'all' : 'neighborhood'))
          }
          onCenterNode={focusNodeOnCanvas}
          onClear={() => {
            setSelectedNodeName(null);
            setSelectedEdgeKey(null);
          }}
        />
      </div>

      {/* Bottom-Right Camera Controls */}
      <div
        className={cn(
          'pointer-events-none absolute bottom-3 z-20 flex items-center',
          rightOffsetClass,
        )}
      >
        <div className="pointer-events-auto flex items-center gap-0.5 rounded-2xl border border-border/70 bg-card/84 p-1 shadow-xl ring-1 ring-white/[0.05] backdrop-blur-2xl">
          <button
            type="button"
            onClick={() => graphControlsRef.current?.zoomIn()}
            className="rounded-xl p-1.5 text-muted-foreground transition hover:bg-muted hover:text-foreground"
            title="Zoom in (+)"
            aria-label="Zoom in"
          >
            <ZoomIn className="h-3.5 w-3.5" />
          </button>
          <button
            type="button"
            onClick={() => graphControlsRef.current?.zoomOut()}
            className="rounded-xl p-1.5 text-muted-foreground transition hover:bg-muted hover:text-foreground"
            title="Zoom out (-)"
            aria-label="Zoom out"
          >
            <ZoomOut className="h-3.5 w-3.5" />
          </button>
          <button
            type="button"
            onClick={() => graphControlsRef.current?.fitToView()}
            className="rounded-xl p-1.5 text-muted-foreground transition hover:bg-muted hover:text-foreground"
            title="Fit universe to view (0)"
            aria-label="Fit graph to view"
          >
            <Maximize2 className="h-3.5 w-3.5" />
          </button>
          <button
            type="button"
            onClick={() => graphControlsRef.current?.resetLayout()}
            className="rounded-xl p-1.5 text-muted-foreground transition hover:bg-muted hover:text-foreground"
            title="Unpin & reset layout"
            aria-label="Reset graph layout"
          >
            <RotateCcw className="h-3.5 w-3.5" />
          </button>
        </div>
      </div>

      {/* Vault Drawer (Stored Facts or Markdown Snapshot) */}
      {vaultDrawer !== 'none' && (
        <aside
          aria-label={vaultDrawer === 'facts' ? 'Stored cognitive facts' : 'Markdown graph snapshot'}
          className={cn(
            'pointer-events-auto absolute bottom-3 top-3 z-30 flex w-[min(460px,calc(100vw-4.5rem))] flex-col overflow-hidden rounded-2xl border border-border/75 bg-card/92 shadow-2xl ring-1 ring-white/[0.06] backdrop-blur-2xl',
            rightOffsetClass,
          )}
        >
          <div className="flex items-center gap-2 border-b border-border/55 px-4 py-3">
            {vaultDrawer === 'facts' ? (
              <Database className="h-4 w-4 text-primary" />
            ) : (
              <FileCode className="h-4 w-4 text-primary" />
            )}
            <h2 className="text-xs font-bold text-foreground">
              {vaultDrawer === 'facts'
                ? `Curated Core Facts (${filteredFacts.length})`
                : 'Core Knowledge Snapshot (.md)'}
            </h2>

            <div className="ml-auto flex items-center gap-1.5">
              {vaultDrawer === 'markdown' && (
                <>
                  <button
                    type="button"
                    onClick={handleCopyMarkdown}
                    className="inline-flex items-center gap-1 rounded-lg border border-border/70 bg-muted/30 px-2.5 py-1 text-[10px] font-semibold text-foreground transition hover:bg-muted"
                  >
                    {copied ? (
                      <>
                        <Check className="h-3 w-3 text-success" /> Copied
                      </>
                    ) : (
                      <>
                        <Copy className="h-3 w-3" /> Copy
                      </>
                    )}
                  </button>
                  <button
                    type="button"
                    onClick={handleDownloadMarkdown}
                    className="inline-flex items-center gap-1 rounded-lg border border-border/70 bg-muted/30 px-2.5 py-1 text-[10px] font-semibold text-foreground transition hover:bg-muted"
                  >
                    <Download className="h-3 w-3" /> .md
                  </button>
                </>
              )}
              <button
                type="button"
                onClick={() => setVaultDrawer('none')}
                className="rounded-lg p-1.5 text-muted-foreground transition hover:bg-muted hover:text-foreground"
                aria-label="Close vault drawer"
              >
                <X className="h-4 w-4" />
              </button>
            </div>
          </div>

          {vaultDrawer === 'facts' ? (
            <div className="flex min-h-0 flex-1 flex-col p-3.5">
              <div className="mb-3 grid grid-cols-2 gap-2">
                <select
                  value={factTagFilter}
                  onChange={(e) => setFactTagFilter(e.target.value)}
                  aria-label="Filter facts by tag"
                  className="h-8 rounded-xl border border-border/65 bg-background/75 px-2.5 text-[11px] text-foreground focus:outline-none focus:ring-1 focus:ring-primary/50"
                >
                  <option value="all">All tags ({factTags.length})</option>
                  {factTags.map((tag) => (
                    <option key={tag} value={tag}>
                      {tag}
                    </option>
                  ))}
                </select>
                <select
                  value={sortMode}
                  onChange={(e) => setSortMode(e.target.value as 'importance' | 'newest')}
                  aria-label="Sort stored facts"
                  className="h-8 rounded-xl border border-border/65 bg-background/75 px-2.5 text-[11px] text-foreground focus:outline-none focus:ring-1 focus:ring-primary/50"
                >
                  <option value="importance">Sort: Importance</option>
                  <option value="newest">Sort: Newest</option>
                </select>
              </div>

              <div className="min-h-0 flex-1 space-y-2 overflow-y-auto pr-1 scrollbar-thin">
                {filteredFacts.length === 0 ? (
                  <div className="rounded-2xl border border-warning/25 bg-warning/8 p-4 text-xs leading-relaxed">
                    <div className="flex items-start gap-2.5">
                      <AlertCircle className="mt-0.5 h-4 w-4 shrink-0 text-warning" />
                      <div className="space-y-1.5">
                        <div className="font-semibold text-warning">No curated facts matched</div>
                        <p className="text-[11px] text-muted-foreground">
                          OpenZ stores durable learnings asynchronously into{' '}
                          <code className="font-mono">{memoryDbPath}</code>.
                        </p>
                      </div>
                    </div>
                  </div>
                ) : (
                  filteredFacts.map((fact) => (
                    <div
                      key={`${fact.timestamp}-${fact.text}`}
                      className="rounded-xl border border-border/70 bg-background/60 p-3 transition hover:border-primary/35"
                    >
                      <div className="mb-1.5 flex flex-wrap items-center gap-1.5">
                        <span className="rounded border border-primary/25 bg-primary/12 px-1.5 py-0.5 font-mono text-[9px] font-semibold text-primary">
                          Importance {fact.importance.toFixed(2)}
                        </span>
                        <span className="ml-auto font-mono text-[9px] text-muted-foreground">
                          {fact.timestamp}
                        </span>
                      </div>
                      <p className="break-words font-sans text-xs leading-relaxed text-foreground">
                        {fact.text}
                      </p>
                    </div>
                  ))
                )}
              </div>
            </div>
          ) : (
            <div className="min-h-0 flex-1 overflow-y-auto p-4 font-mono text-xs prose prose-xs dark:prose-invert max-w-none text-foreground scrollbar-thin">
              <ReactMarkdown remarkPlugins={[remarkGfm]}>{generateMarkdownString()}</ReactMarkdown>
            </div>
          )}
        </aside>
      )}
    </div>
  );
};
