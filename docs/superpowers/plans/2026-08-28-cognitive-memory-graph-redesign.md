# Cognitive Memory Graph Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign the Cognitive Memory Graph into a readable cluster-first explorer that preserves and exposes all backend entities, including roughly 1,000-node datasets, without synthetic or hardcoded records.

**Architecture:** Keep the existing React/canvas boundary, but move graph math into pure data helpers. `KnowledgeView` owns the authoritative filters/mode/selection state; `ObsidianGraph` receives the complete filtered dataset and renders deterministic clusters, visible nodes, and selected neighborhoods with level-of-detail. A small inspector in `KnowledgeView` presents the selected backend record without adding graph data.

**Tech Stack:** React 19, TypeScript, Tailwind CSS, HTML Canvas 2D, Zustand, existing OpenZ WebSocket payloads.

## Global Constraints

- Do not add WebGL or a new graph-rendering dependency for this slice.
- Graph content comes only from `cognitive_memory` WebSocket payloads; no UI seed arrays, demo entities, fabricated facts, or fallback graph records.
- Keep all nodes/edges available for search, filtering, selection, and export even when the canvas uses level-of-detail rendering.
- Perform layout only when data, viewport, or layout mode changes; do not update React state from the animation loop.
- Use responsive controls and touch-sized targets for small screens.
- Run only `bun run lint`, `bun run build`, and `git diff --check`; use scoped Rust commands only if backend code changes.
- Preserve unrelated dirty-worktree changes.

---

### Task 1: Add pure deterministic graph-layout helpers

**Files:**
- Create: `web/src/components/graphLayout.ts`
- Modify: `web/src/types/openz.ts` only if the exported graph mode/type belongs with shared payload types

**Interfaces:**
- Consumes: `CognitiveNode[]`, `CognitiveEdge[]`, canvas width/height, current zoom mode, and optional selected node.
- Produces: `GraphMode`, `LayoutNode`, `GraphCluster`, `VisibleGraph`, and pure helper functions used by `ObsidianGraph`.

- [ ] **Step 1: Define the data-only interfaces**

```ts
export type GraphMode = 'overview' | 'all' | 'neighborhood';

export interface LayoutNode {
  id: string;
  name: string;
  type: string;
  observations: string;
  degree: number;
  x: number;
  y: number;
  radius: number;
  color: string;
}

export interface GraphCluster {
  id: string;
  type: string;
  nodeIds: string[];
  x: number;
  y: number;
  radius: number;
  representativeIds: string[];
}

export interface VisibleGraph {
  nodes: LayoutNode[];
  edges: CognitiveEdge[];
  clusters: GraphCluster[];
  loadedNodeCount: number;
  visibleNodeCount: number;
  visibleEdgeCount: number;
}
```

- [ ] **Step 2: Implement stable identity and layout helpers**

Implement these exact functions without `Math.random()`:

```ts
export function stableHash(value: string): number;
export function stablePoint(id: string, width: number, height: number, padding: number): { x: number; y: number };
export function computeDegrees(nodes: CognitiveNode[], edges: CognitiveEdge[]): Map<string, number>;
export function buildClusters(nodes: CognitiveNode[], edges: CognitiveEdge[], width: number, height: number): GraphCluster[];
export function layoutNodes(nodes: CognitiveNode[], edges: CognitiveEdge[], width: number, height: number): LayoutNode[];
export function selectVisibleGraph(
  nodes: LayoutNode[],
  edges: CognitiveEdge[],
  clusters: GraphCluster[],
  mode: GraphMode,
  zoom: number,
  viewport: { left: number; top: number; right: number; bottom: number },
  selectedId?: string | null,
  searchQuery?: string,
): VisibleGraph;
```

Use union-find for connected components in `buildClusters`; place isolated nodes into deterministic type buckets. Use stable hash-derived angles/radii for initial points, then compute cluster centers from member positions. In `selectVisibleGraph`, overview mode returns cluster representatives plus selected/search matches, all mode returns viewport-visible nodes, and neighborhood mode returns the selected node plus its connected endpoints. Always return original edge records only when both endpoint IDs are visible.

- [ ] **Step 3: Verify helper behavior with a TypeScript-only smoke script**

Run:

```bash
cd web
bun run build
```

Expected: TypeScript accepts the new interfaces and functions; no runtime dependencies are added.

- [ ] **Step 4: Commit the isolated helper change**

```bash
git add web/src/components/graphLayout.ts web/src/types/openz.ts
git commit -m "perf: add deterministic cognitive graph layout helpers"
```

### Task 2: Refactor `ObsidianGraph` for cluster-first rendering

**Files:**
- Modify: `web/src/components/ObsidianGraph.tsx`

**Interfaces:**
- Consumes: helpers from Task 1, backend-provided `CognitiveNode[]`/`CognitiveEdge[]`, and controlled props from `KnowledgeView`.
- Produces: a canvas that renders all backend records through level-of-detail plus callbacks for selected node and graph mode.

- [ ] **Step 1: Extend the component props and remove the progressive stream contract**

Add these props while keeping existing optional callbacks compatible:

```ts
mode?: GraphMode;
searchQuery?: string;
selectedNodeName?: string | null;
onModeChange?: (mode: GraphMode) => void;
onVisibleStatsChange?: (stats: { loaded: number; visible: number; edges: number }) => void;
```

Delete the one-node-at-a-time interval, `streamProgress`, random radial spawn positions, and “Load All” action. Initialize the layout ref from `layoutNodes()` whenever the input node/edge identity set or canvas dimensions changes. Preserve positions by node ID when only observations/degrees update.

- [ ] **Step 2: Replace the O(n²) always-on simulation with bounded layout settling**

Use a layout ref containing `LayoutNode[]`, cluster data, and a `layoutVersion`. Run at most 12 bounded relaxation frames after a layout update, then stop physics. Each frame should process only visible/nearby nodes returned by `selectVisibleGraph`; do not compare every pair of 1,000 nodes. Keep drag/pin behavior by updating the layout ref and reheating only the selected neighborhood.

- [ ] **Step 3: Implement level-of-detail drawing**

In the canvas render loop:

```ts
const visible = selectVisibleGraph(
  layout.nodes,
  graphData.edges,
  layout.clusters,
  mode,
  transform.k,
  viewport,
  selectedNodeRef.current?.id,
  searchQuery,
);
```

Draw cluster halos and representative counts in overview mode, individual visible edges before nodes, and individual nodes only for visible/search/selected neighborhood records. Draw labels only when `transform.k > 1.15`, the node is selected/hovered/search-matched, or its degree is in the top visible percentile. Limit animated particles to selected edges and a small visible-edge budget. Publish `onVisibleStatsChange` at most once per layout/search/mode change, never per animation frame.

- [ ] **Step 4: Add stable selection and viewport behavior**

Update hit testing to check visible nodes first, then cluster representatives. Selecting a cluster zooms to its bounds; selecting a node calls `onSelectNode(name)`. Search matches must be reachable even if their node is outside the current viewport by centering the first match. Fit-to-view must use cluster bounds in overview and node bounds in all/neighborhood modes.

- [ ] **Step 5: Replace the graph overlay controls**

Keep zoom, fit, and reset actions, but add a compact segmented mode control (`Overview`, `All nodes`, `Neighborhood`) and a loaded/visible count chip. Remove “Streaming entity…” copy and any text implying that only a subset was loaded. Ensure the legend is generated from actual `graphData.nodes` types and remains scrollable when many types exist.

- [ ] **Step 6: Run the WebUI checks**

Run:

```bash
cd web
bun run lint
bun run build
```

Expected: both commands pass; build may report the existing large-chunk warning but must not fail.

- [ ] **Step 7: Commit the renderer change**

```bash
git add web/src/components/ObsidianGraph.tsx
git commit -m "perf: render cognitive graph with level of detail"
```

### Task 3: Redesign `KnowledgeView` as overview → explore → inspect

**Files:**
- Modify: `web/src/components/KnowledgeView.tsx`

**Interfaces:**
- Consumes: `CognitiveMemoryStats`, `RuntimeInventory`, and graph callbacks from Task 2.
- Produces: a responsive page with trustworthy status, mode controls, filtered totals, selected-node inspector, and unchanged markdown/JSON export semantics.

- [ ] **Step 1: Add page-level graph state and derive inspection data**

Add:

```ts
const [graphMode, setGraphMode] = useState<GraphMode>('overview');
const [selectedNodeName, setSelectedNodeName] = useState<string | null>(null);
const [lastSyncedAt, setLastSyncedAt] = useState<number | null>(null);
```

Set `lastSyncedAt` in an effect keyed by the cognitive payload (`cognitiveStats.nodes`, `cognitiveStats.edges`, and `cognitiveStats.facts`) so it changes only after a gateway response updates the store. Derive `selectedNode`, `selectedNodeEdges`, and `selectedNeighbors` from the complete backend arrays, not just currently visible canvas nodes. Clear selection when the selected record disappears after a refresh.

- [ ] **Step 2: Replace the dense header/filter block with a responsive status toolbar**

Use a two-row layout: title/status on top, controls below. Show live totals, filtered totals, database existence, and “`visible / loaded` nodes” from the renderer callback. Keep search input and type/fact filters, but make each control at least `min-h-10` on narrow screens. Add buttons for `Overview`, `All nodes`, and `Neighborhood`; disable Neighborhood until a node is selected. Keep clear-filters and sync actions.

- [ ] **Step 3: Add a selected-entity inspector beside/below the graph**

Render a panel when `selectedNode` exists:

```tsx
<aside aria-label="Selected memory entity">
  <p>{selectedNode.entity_type}</p>
  <h2>{selectedNode.name}</h2>
  <p>{observationText(selectedNode.observations)}</p>
  <p>{selectedNodeEdges.length} relations</p>
  {/* connected entities and relation types */}
</aside>
```

On desktop use a two-column graph/inspector grid; on mobile stack the inspector below the graph. Add “Focus neighborhood” and “Clear selection” controls. Do not insert any fallback entity text.

- [ ] **Step 4: Pass complete data and controlled state to `ObsidianGraph`**

Pass `filteredNodes`/`filteredEdges` after page filters, plus `graphMode`, `searchQuery`, `selectedNodeName`, `onModeChange`, and `onSelectNode`. Keep filtered/backend totals visually distinct so a 100-node viewport does not imply that 900 records were lost. Keep exports sourced from the full filtered arrays.

- [ ] **Step 5: Make empty/partial states explicit**

When no records are received, show the actual runtime paths and a “No persisted entities found” explanation. When the database exists but a query returns no records, display zero rather than personal/location examples. When search has no match, show a no-match message and reset action while preserving the underlying graph.

- [ ] **Step 6: Run checks and inspect the generated bundle**

Run:

```bash
cd web
bun run lint
bun run build
git diff --check
```

Expected: no lint/type errors and no diff whitespace errors.

- [ ] **Step 7: Commit the page redesign**

```bash
git add web/src/components/KnowledgeView.tsx
git commit -m "feat: redesign cognitive memory graph explorer"
```

### Task 4: Large-dataset and hardcoded-data verification

**Files:**
- Modify: `web/src/components/ObsidianGraph.tsx` or `web/src/components/graphLayout.ts` only if verification reveals a missed render path.
- Modify: `web/src/components/KnowledgeView.tsx` only if verification reveals a missed fallback string.

**Interfaces:**
- Consumes: final graph page and helper behavior from Tasks 1–3.
- Produces: a verified graph with all records addressable and no synthetic personal/location records.

- [ ] **Step 1: Search affected source for forbidden demo data and synthetic graph construction**

Run:

```bash
rg -n -i "aswin|bangalore|bengaluru|demo|mock|sample|Fact:|#tag|tagged_with|synthetic|seed" web/src/components/KnowledgeView.tsx web/src/components/ObsidianGraph.tsx web/src/components/graphLayout.ts
```

Expected: no runtime data arrays or synthetic-node creation. UX copy may mention “demo” only if it is clearly an error explanation; remove it if it can appear as graph content.

- [ ] **Step 2: Exercise a generated 1,000-node payload without a browser dependency**

Use a temporary Bun script that imports `stablePoint`, `layoutNodes`, `buildClusters`, and `selectVisibleGraph`, creates 1,000 deterministic nodes plus 2,000 edges, and asserts:

```ts
if (layout.length !== 1000) throw new Error('layout dropped nodes');
if (visible.loadedNodeCount !== 1000) throw new Error('loaded count mismatch');
if (layout.some((node) => !Number.isFinite(node.x) || !Number.isFinite(node.y))) throw new Error('invalid position');
```

Run it with `bun` and delete the temporary script after it passes; do not add a test dependency.

- [ ] **Step 3: Run final scoped verification**

Run:

```bash
cd web
bun run lint
bun run build
cd ..
git diff --check
```

Expected: all pass. Do not run full Cargo check/build/test.

- [ ] **Step 4: Update the existing remediation TODO**

Mark the Cognitive Memory Graph redesign/performance items complete in `plan/webui-remediation-todo.md`, and add the new helper/rendering files to the page-repair checklist.

