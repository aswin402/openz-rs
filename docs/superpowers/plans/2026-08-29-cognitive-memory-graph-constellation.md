# Cognitive Memory Graph Constellation UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Upgrade the Cognitive Memory Graph into a deterministic universe-like constellation atlas with Graphify-inspired degree, community, and relation-confidence semantics backed only by live OpenZ records.

**Architecture:** Extend the WebSocket graph payload with the persisted edge confidence/timestamp fields already present in `graph_edges`, normalize graph semantics in a focused frontend helper, and keep complete data separate from the renderer's bounded visible slice. Refine the existing canvas renderer into a cosmic constellation atlas and integrate filters, relation details, and a responsive inspector in `KnowledgeView`.

**Tech Stack:** Rust/Axum WebSocket endpoint, React 19, TypeScript, Vite, Tailwind CSS, Lucide icons, Canvas 2D, Bun test runner.

## Global Constraints

- Use live persisted OpenZ cognitive-memory records only; never add synthetic entities, names, locations, observations, relationships, or provenance.
- Preserve the full node/edge arrays in page state; visual level-of-detail may bound what is drawn, not what is loaded or searchable.
- Layout and decorative star positions must be deterministic from stable identifiers and graph structure; do not use `Math.random()` or time-based layout positions.
- Keep canvas work bounded with viewport culling, degree/zoom label thresholds, and adaptive glow/particle effects.
- Missing optional confidence/provenance/source data must render as `Not recorded`.
- Do not add dependencies unless an existing project dependency cannot satisfy the requirement.
- Run only scoped verification: `bun run lint`, `bun run build`, and focused `bun test`/Bun smoke checks. Do not run full Cargo check/build/test commands.

---

### Task 1: Expose persisted edge semantics and type them in the WebUI

**Files:**
- Modify: `src/channels/websocket.rs:3245-3270` (active graph-edge query in `fetch_real_cognitive_memory`)
- Modify: `web/src/types/openz.ts:123-127` (`CognitiveEdge`)
- Modify: `web/src/store/useOpenZStore.ts:1248-1267` (cognitive-memory payload normalization)

**Interfaces:**
- Consumes: existing `graph_edges` columns `from_name`, `to_name`, `relation_type`, `confidence`, and `valid_from`.
- Produces: `CognitiveEdge` objects with optional `confidence?: number` and `valid_from?: string`; code-call and other edges that do not expose these fields remain valid and display `Not recorded`.

- [x] **Step 1: Add focused backend response fields**

Change the active graph-edge query to select persisted confidence and creation time, while preserving the current filtering rule:

```rust
conn.prepare(
    "SELECT from_name, to_name, relation_type, confidence, valid_from
     FROM graph_edges WHERE valid_until IS NULL",
)?
```

Map the additional columns without manufacturing values:

```rust
Ok(serde_json::json!({
    "from_name": r.get::<_, String>(0)?,
    "to_name": r.get::<_, String>(1)?,
    "relation_type": r.get::<_, String>(2)?,
    "confidence": r.get::<_, f64>(3)?,
    "valid_from": r.get::<_, String>(4)?,
}))
```

Leave code-call edges on their existing three-field shape because their source table has no confidence column.

- [x] **Step 2: Extend the shared TypeScript contract**

Update `CognitiveEdge` without making optional backend fields required:

```ts
export interface CognitiveEdge {
  from_name: string;
  to_name: string;
  relation_type: string;
  confidence?: number;
  valid_from?: string;
  provenance?: string;
  source?: string;
}
```

- [x] **Step 3: Preserve optional fields in store normalization**

When copying `payload.edges`, keep only valid edge records and normalize optional values rather than asserting them:

```ts
edges: Array.isArray(payload.edges)
  ? payload.edges
      .filter((edge) => edge && typeof edge === 'object')
      .map((edge) => ({
        from_name: typeof edge.from_name === 'string' ? edge.from_name : '',
        to_name: typeof edge.to_name === 'string' ? edge.to_name : '',
        relation_type: typeof edge.relation_type === 'string' ? edge.relation_type : '',
        ...(typeof edge.confidence === 'number' ? { confidence: edge.confidence } : {}),
        ...(typeof edge.valid_from === 'string' ? { valid_from: edge.valid_from } : {}),
        ...(typeof edge.provenance === 'string' ? { provenance: edge.provenance } : {}),
        ...(typeof edge.source === 'string' ? { source: edge.source } : {}),
      }))
      .filter((edge) => edge.from_name.length > 0 && edge.to_name.length > 0)
  : [],
```

- [x] **Step 4: Run the focused type check**

Run from `web/`:

```bash
bun run build
```

Expected: TypeScript compilation succeeds; Vite may print only its existing chunk-size warning.

- [x] **Step 5: Commit**

```bash
git add src/channels/websocket.rs web/src/types/openz.ts web/src/store/useOpenZStore.ts
git commit -m "feat: expose cognitive edge confidence"
```

### Task 2: Create a focused graph-semantics helper and tests

**Files:**
- Create: `web/src/components/graphSemantics.ts`
- Create: `web/src/components/graphSemantics.test.ts`

**Interfaces:**
- Consumes: `CognitiveNode[]` and `CognitiveEdge[]` from `web/src/types/openz.ts`.
- Produces: `GraphSemantics`, `NodeMetrics`, and `EdgeSemantics` used by both `KnowledgeView` and `ObsidianGraph`.

Define these exact exported shapes:

```ts
export type EdgeProvenance = 'extracted' | 'inferred' | 'not_recorded';

export interface NodeMetrics {
  degree: number;
  importance: number;
  clusterId: string;
}

export interface EdgeSemantics {
  edge: CognitiveEdge;
  confidence: number | null;
  provenance: EdgeProvenance;
  source: string | null;
}

export interface GraphSemantics {
  nodeMetrics: Map<string, NodeMetrics>;
  edgeMetrics: Map<string, EdgeSemantics>;
  clusterIds: string[];
  malformedRecords: number;
}

export function buildGraphSemantics(nodes: CognitiveNode[], edges: CognitiveEdge[]): GraphSemantics;
export function edgeKey(edge: Pick<CognitiveEdge, 'from_name' | 'to_name' | 'relation_type'>): string;
export function formatConfidence(value: number | null): string;
export function formatProvenance(value: EdgeProvenance): string;
```

- [x] **Step 1: Write failing semantic tests**

Add Bun tests that establish the required behavior:

```ts
import { describe, expect, test } from 'bun:test';
import { buildGraphSemantics, edgeKey, formatConfidence } from './graphSemantics';

describe('graph semantics', () => {
  test('computes degree and bounded importance from real edges', () => {
    const nodes = [
      { name: 'hub', entity_type: 'concept', observations: '[]' },
      { name: 'leaf', entity_type: 'concept', observations: '[]' },
    ];
    const edges = [{ from_name: 'hub', to_name: 'leaf', relation_type: 'uses' }];
    const semantics = buildGraphSemantics(nodes, edges);
    expect(semantics.nodeMetrics.get('hub')?.degree).toBe(1);
    expect(semantics.nodeMetrics.get('hub')?.importance).toBeGreaterThanOrEqual(0);
    expect(semantics.nodeMetrics.get('hub')?.importance).toBeLessThanOrEqual(1);
  });

  test('keeps missing provenance explicit and preserves confidence', () => {
    const edge = { from_name: 'a', to_name: 'b', relation_type: 'links', confidence: 0.7 };
    const semantics = buildGraphSemantics(
      [
        { name: 'a', entity_type: 'concept', observations: '[]' },
        { name: 'b', entity_type: 'concept', observations: '[]' },
      ],
      [edge],
    );
    const detail = semantics.edgeMetrics.get(edgeKey(edge));
    expect(detail?.confidence).toBe(0.7);
    expect(detail?.provenance).toBe('not_recorded');
    expect(formatConfidence(null)).toBe('Not recorded');
  });

  test('rejects malformed endpoints without losing valid records', () => {
    const semantics = buildGraphSemantics(
      [{ name: 'a', entity_type: 'concept', observations: '[]' }],
      [{ from_name: 'a', to_name: '', relation_type: 'broken' }],
    );
    expect(semantics.malformedRecords).toBe(1);
    expect(semantics.edgeMetrics.size).toBe(0);
  });
});
```

- [x] **Step 2: Run the tests to verify the initial failure**

Run from `web/`:

```bash
bun test src/components/graphSemantics.test.ts
```

Expected: FAIL because `graphSemantics.ts` and its exported functions do not yet exist.

- [x] **Step 3: Implement the minimal deterministic helper**

Use endpoint degree from deduplicated valid edges, normalize confidence to [0, 1] only when the backend gives a finite number, map only explicit provenance values to extracted/inferred, and otherwise return not_recorded. Compute connected components over valid endpoints, collapse degree-zero singleton components by normalized entity type (matching buildClusters), sort each component's names, and assign cluster:${stableHash(sortedNames.join('|')).toString(16)}. Do not add random layout state.

The importance value must be bounded and monotonic with degree:

```ts
const maxDegree = Math.max(...degrees.values(), 0);
const importance = maxDegree === 0 ? 0 : Math.min(1, degree / maxDegree);
```

- [x] **Step 4: Run the focused semantic tests**

Run:

```bash
bun test src/components/graphSemantics.test.ts
```

Expected: all semantic tests pass.

- [x] **Step 5: Commit**

```bash
git add web/src/components/graphSemantics.ts web/src/components/graphSemantics.test.ts
git commit -m "feat: add graph semantics metadata"
```

### Task 3: Refine the canvas into a cosmic constellation atlas

**Files:**
- Modify: `web/src/components/ObsidianGraph.tsx`
- Modify: `web/src/components/graphLayout.ts`

**Interfaces:**
- Consumes: `GraphSemantics`, deterministic `LayoutNode[]`, and complete live `CognitiveEdge[]`.
- Produces: a bounded canvas renderer with constellation halos, degree-aware stars, semantic link highlighting, and selection focus callbacks compatible with the existing `ObsidianGraphProps`.

- [x] **Step 1: Add deterministic space background primitives**

Create a small fixed-count decorative star list from `stableHash('space-star-' + index)` and canvas dimensions. Store it in a ref keyed by the current canvas size; never use graph entity names for decorative records and never use `Math.random()`.

Render a radial deep-space gradient, sparse stars, and a low-contrast coordinate grid. Skip decorative stars while the canvas is resizing or when `prefers-reduced-motion` is active.

- [x] **Step 2: Add constellation halo and degree-aware node styling**

For overview mode, draw each `GraphCluster` as a soft, layered nebula halo with a thin dashed perimeter and a label containing only the derived type/community and real member count. For entity nodes:

```ts
const radius = 4 + Math.min(12, Math.sqrt(node.degree) * 2.2);
const glowRadius = radius * (node.isSelected ? 4 : node.degree > 0 ? 2 : 1.2);
```

Use the existing stable type palette for color and add a brightness/alpha tier from `NodeMetrics.importance`. Keep the radius capped and draw labels only for selection, search matches, connected neighbors, or high-importance nodes above the current zoom threshold.

- [x] **Step 3: Add semantic relation rendering**

Draw directed relation paths with a small arrowhead when zoom permits. Use confidence only when present to vary opacity; use a neutral dashed style for `not_recorded` provenance. Hovered/selected relations use the selected node color, while unrelated links fade. Keep the existing per-frame edge cap and disable moving particles when the visible edge count is dense.

- [x] **Step 4: Improve constellation and node selection focus**

Add a deterministic focus helper that pans the selected node or clicked cluster toward the canvas center over a short bounded interpolation. On selected nodes, draw concentric orbital rings around direct neighbors and retain the existing neighborhood mode. A cluster click switches to `all` mode and fits only that cluster's bounds.

- [x] **Step 5: Keep rendering bounded and accessible**

Continue viewport culling before drawing. Keep DOM controls keyboard reachable, add `aria-pressed` to mode/settings toggles, and expose a concise canvas status label such as `1,000 records loaded; 84 visible; 312 visible relations` through `aria-label` or an adjacent live region. Do not put all node labels into the DOM.

- [x] **Step 6: Run the focused graph smoke check**

Run from `web/`:

```bash
bun -e 'import { buildClusters, layoutNodes, selectVisibleGraph } from "./src/components/graphLayout.ts"; const nodes=Array.from({length:1000},(_,i)=>({name:`entity-${i}`,entity_type:i%4===0?"person":"concept",observations:"[]"})); const edges=Array.from({length:1400},(_,i)=>({from_name:`entity-${i%1000}`,to_name:`entity-${(i*17+3)%1000}`,relation_type:"related"})); const clusters=buildClusters(nodes,edges); const laid=layoutNodes(nodes,edges,clusters,1200,760); const view=selectVisibleGraph(laid,edges,clusters,"overview",1,{left:-10000,top:-10000,right:10000,bottom:10000}); if(view.nodes.length!==78||clusters.length!==13||laid.length!==1000) throw new Error("unexpected graph bounds"); console.log(`large graph smoke passed: ${laid.length} nodes, ${clusters.length} clusters, ${view.nodes.length} overview nodes`);'
```

Expected: `large graph smoke passed: 1000 nodes, 13 clusters, 78 overview nodes`.

- [x] **Step 7: Commit**

```bash
git add web/src/components/ObsidianGraph.tsx web/src/components/graphLayout.ts
git commit -m "feat: render cognitive graph as constellation atlas"
```

### Task 4: Integrate filters, relation details, and responsive inspector

**Files:**
- Modify: `web/src/components/KnowledgeView.tsx:126-620`
- Modify: `web/src/components/ObsidianGraph.tsx` (prop wiring for filter state and selected relation)

**Interfaces:**
- Consumes: `GraphSemantics` and the complete cognitive-memory arrays from `useOpenZStore`.
- Produces: a page-level constellation toolbar/filter rail, relation-aware inspector, and responsive mobile stacking without changing markdown/facts tabs.

- [x] **Step 1: Add page-level semantic filter state**

Add controlled state with explicit defaults:

```ts
const [communityFilter, setCommunityFilter] = useState('all');
const [relationTypeFilter, setRelationTypeFilter] = useState('all');
const [mostConnectedOnly, setMostConnectedOnly] = useState(false);
const [selectedEdgeKey, setSelectedEdgeKey] = useState<string | null>(null);
```

Build `GraphSemantics` once from the complete `cognitiveStats.nodes` and `cognitiveStats.edges`, then derive filter options from the returned cluster IDs and relation types. Do not derive options from hardcoded lists.

- [x] **Step 2: Apply filters without truncating source data**

Filter nodes by type, cluster, search, and (when enabled) the top quartile of real degree. Filter edges by relation type and require both endpoints to remain in the filtered graph except when search/selection explicitly reveals an endpoint. Keep `allNodes`/`allEdges` intact for the inspector and export actions.

- [x] **Step 3: Upgrade the inspector relation rows**

For every selected edge, show direction, relation type, confidence, provenance, and source/context fallback:

```tsx
<span>{edge.from_name} → {edge.to_name}</span>
<span>{edge.relation_type}</span>
<span>{formatConfidence(detail?.confidence ?? null)}</span>
<span>{formatProvenance(detail?.provenance ?? 'not_recorded')}</span>
<span>{detail?.source ?? 'Not recorded'}</span>
```

Clicking a relation row sets `selectedEdgeKey`, selects both endpoints for highlighting, and keeps the relation detail visible on narrow screens. If a node has no relations, retain the existing explicit empty state.

- [x] **Step 4: Add responsive filter and status UI**

Place filters in a collapsible/stacking rail on small screens and a compact horizontal rail on desktop. Keep counts live: loaded records, visible records, visible relations, malformed-record warning count, and connection state. Use actual graph-derived labels and hide optional controls when no corresponding data exists.

- [x] **Step 5: Add page-level loading/stale/empty states**

Differentiate `nodes.length === 0` before the first WebSocket response from a confirmed empty graph and from a stale/disconnected response. Never render example nodes. Preserve the last real snapshot while showing a non-blocking stale indicator.

- [x] **Step 6: Run focused tests and WebUI checks**

Run from `web/`:

```bash
bun test src/components/graphSemantics.test.ts
bun run lint
bun run build
```

Expected: semantic tests pass, ESLint reports no errors, and Vite completes successfully with only the known chunk-size warning if it remains.

- [x] **Step 7: Commit**

```bash
git add web/src/components/KnowledgeView.tsx web/src/components/ObsidianGraph.tsx
git commit -m "feat: add graph filters and relation inspector"
```

### Task 5: Final regression and documentation update

**Files:**
- Modify: `plan/webui-remediation-todo.md` (mark the constellation semantics/UI item complete)
- Modify: `docs/superpowers/plans/2026-08-29-cognitive-memory-graph-constellation.md` (check off completed steps during execution)

**Interfaces:**
- Consumes: completed Tasks 1–4 and their focused checks.
- Produces: a traceable remediation record and final verification evidence.

- [x] **Step 1: Run the final focused checks**

Run from the repository root or `web/` as appropriate:

```bash
git diff --check
cd web && bun test src/components/graphSemantics.test.ts && bun run lint && bun run build
```

Expected: no whitespace errors, all focused tests pass, lint passes, and the production build completes. Do not run workspace-wide Cargo commands.

- [x] **Step 2: Scan affected UI files for prohibited fabricated data**

Run from the repository root:

```bash
rg -n -i 'aswin|bangalore|bengaluru|demo|mock|sample|synthetic|placeholder|seed' web/src/components/ObsidianGraph.tsx web/src/components/KnowledgeView.tsx web/src/components/graphLayout.ts web/src/components/graphSemantics.ts || true
```

Expected: no graph-record fixture strings or fabricated entity values in the affected files. Generic prose such as “synthetic graph records” should not be used as data.

- [x] **Step 3: Record the completed TODO item and commit documentation**

Mark the constellation atlas, semantic relation details, and bounded rendering item complete in `plan/webui-remediation-todo.md`, then commit only the plan/todo files:

```bash
git add plan/webui-remediation-todo.md docs/superpowers/plans/2026-08-29-cognitive-memory-graph-constellation.md
git commit -m "docs: track constellation graph remediation"
```

