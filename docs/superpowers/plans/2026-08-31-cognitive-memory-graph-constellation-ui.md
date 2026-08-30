# Cognitive Memory Graph Constellation UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Cognitive Memory Graph look and behave like a dense connected constellation while keeping every gateway entity searchable, selectable, and performant at approximately 1,000 nodes.

**Architecture:** Keep the existing canvas-based `ObsidianGraph` and split responsibilities cleanly: `graphLayout.ts` decides deterministic placement, viewport visibility, and edge level-of-detail; `ObsidianGraph.tsx` renders the observatory visual system and interactions; `KnowledgeView.tsx` owns filters, totals, and the inspector. The overview will represent all loaded nodes at low detail, while a deterministic edge-priority pass limits expensive line/pulse work and preserves selected/high-value relationships.

**Tech Stack:** React 19, TypeScript, Vite, HTML Canvas 2D, Tailwind CSS, Bun test.

## Global Constraints

- Every graph entity and relationship must originate from the gateway payload; no demo names, locations, facts, or synthetic records.
- Default overview must represent every loaded entity; viewport culling may reduce what is currently visible after panning.
- Keep canvas device-pixel ratio capped at 2 and stop settling after a bounded frame count.
- Preserve stable positions across gateway refreshes and keep search/selection able to reveal off-viewport matches.
- Respect `prefers-reduced-motion` by disabling particles and continuous settling while retaining pan/zoom.
- Use only focused WebUI verification: `bun test`, `bun run lint`, and `bun run build`; do not run full-workspace Cargo commands.
- Preserve unrelated dirty-worktree edits and stage only graph files, tests, and this plan.

---

### Task 1: Make overview selection represent the complete graph

**Files:**
- Modify: `web/src/components/graphLayout.ts:27-35,222-300`
- Create: `web/src/components/graphLayout.test.ts`
- Modify: `web/src/components/ObsidianGraph.tsx:20-30,625-645`

**Interfaces:**
- `selectVisibleGraph(...)` continues returning `VisibleGraph`, but `VisibleGraph.visibleEdgeCount` is the number of valid edges after node/filter selection, not the number eventually drawn.
- Add `prioritizeGraphEdges(edges: CognitiveEdge[], nodes: LayoutNode[], selectedId: string | null, zoom: number, maxEdges?: number): CognitiveEdge[]` to `graphLayout.ts`. It returns at most `maxEdges` edges in deterministic priority order.

- [ ] **Step 1: Write failing data-only tests**

```ts
import { describe, expect, test } from 'bun:test';
import type { CognitiveEdge, CognitiveNode } from '../types/openz';
import { buildClusters, layoutNodes, prioritizeGraphEdges, selectVisibleGraph } from './graphLayout';

const node = (name: string, type = 'concept'): CognitiveNode => ({ name, entity_type: type, observations: '[]' });
const edge = (from_name: string, to_name: string, relation_type = 'links', confidence?: number): CognitiveEdge => ({
  from_name,
  to_name,
  relation_type,
  ...(confidence === undefined ? {} : { confidence }),
});

test('overview represents every loaded node instead of only cluster representatives', () => {
  const nodes = Array.from({ length: 24 }, (_, index) => node(`entity-${index}`));
  const edges = nodes.slice(1).map((item, index) => edge('entity-0', item.name, 'relates'));
  const layout = layoutNodes(nodes, edges, 900, 560);
  const result = selectVisibleGraph(layout, edges, buildClusters(nodes, edges, 900, 560), 'overview', 0.45, {
    left: -1000, top: -1000, right: 2000, bottom: 2000,
  });
  expect(result.loadedNodeCount).toBe(24);
  expect(result.nodes).toHaveLength(24);
});

test('edge priority preserves selected neighborhood before low-value edges', () => {
  const nodes = [node('hub'), ...Array.from({ length: 8 }, (_, index) => node(`leaf-${index}`))];
  const edges = [
    ...nodes.slice(1).map((item) => edge('hub', item.name, 'primary', 0.95)),
    edge('leaf-0', 'leaf-1', 'incidental', 0.1),
  ];
  const layout = layoutNodes(nodes, edges, 900, 560);
  const selected = prioritizeGraphEdges(edges, layout, 'hub', 0.4, 3);
  expect(selected).toHaveLength(3);
  expect(selected.every((item) => item.from_name === 'hub' || item.to_name === 'hub')).toBe(true);
});
```

- [ ] **Step 2: Run the focused tests and verify the red phase**

Run: `bun test ./src/components/graphLayout.test.ts`

Expected: FAIL because overview currently reveals only representatives and `prioritizeGraphEdges` does not exist.

- [ ] **Step 3: Implement complete-node selection and deterministic edge priority**

In `selectVisibleGraph`, replace the overview representative-only branch with viewport-aware insertion of every node. Keep search and selected-neighbor insertion after the mode branch so a matching node is still added even when it is outside the viewport. Add `prioritizeGraphEdges` with this ordering:

```ts
const degree = new Map(nodes.map((item) => [item.id, item.degree]));
const selected = selectedId ? new Set(edges.filter((item) => item.from_name === selectedId || item.to_name === selectedId)) : new Set();
return [...edges]
  .map((item, index) => ({ item, index }))
  .sort((left, right) => {
    const leftSelected = selected.has(left.item) ? 1 : 0;
    const rightSelected = selected.has(right.item) ? 1 : 0;
    const leftConfidence = left.item.confidence ?? 0;
    const rightConfidence = right.item.confidence ?? 0;
    const leftDegree = (degree.get(left.item.from_name) ?? 0) + (degree.get(left.item.to_name) ?? 0);
    const rightDegree = (degree.get(right.item.from_name) ?? 0) + (degree.get(right.item.to_name) ?? 0);
    const zoomBoost = zoom > 1 ? zoom : 1;
    return rightSelected - leftSelected || (rightConfidence - leftConfidence) * zoomBoost || rightDegree - leftDegree || left.index - right.index;
  })
  .slice(0, Math.max(0, maxEdges))
  .map(({ item }) => item);
```

Use the helper in the draw loop before assigning `visibleEdgesRef.current`; do not change the authoritative `visibleEdgeCount` returned by `selectVisibleGraph`.

- [ ] **Step 4: Run the focused tests and verify the green phase**

Run: `bun test ./src/components/graphLayout.test.ts`

Expected: PASS with both tests passing.

- [ ] **Step 5: Commit the selection contract**

```bash
git add web/src/components/graphLayout.ts web/src/components/graphLayout.test.ts web/src/components/ObsidianGraph.tsx
git commit -m "feat: represent full graph in constellation overview"
```

### Task 2: Improve deterministic constellation placement and community hierarchy

**Files:**
- Modify: `web/src/components/graphLayout.ts:90-218`
- Modify: `web/src/components/graphLayout.test.ts`

**Interfaces:**
- Keep `buildClusters`, `layoutNodes`, and `GraphCluster` public signatures stable.
- Add a private deterministic helper `clusterPoint(cluster: GraphCluster, node: CognitiveNode, degree: number, width: number, height: number): { x: number; y: number }`.

- [ ] **Step 1: Add failing placement tests**

```ts
test('places a connected hub nearer the community center than leaves', () => {
  const nodes = [node('hub'), node('leaf-a'), node('leaf-b'), node('leaf-c')];
  const edges = [edge('hub', 'leaf-a'), edge('hub', 'leaf-b'), edge('hub', 'leaf-c')];
  const clusters = buildClusters(nodes, edges, 900, 560);
  const layout = layoutNodes(nodes, edges, 900, 560);
  const cluster = clusters[0];
  const hub = layout.find((item) => item.id === 'hub')!;
  const leaf = layout.find((item) => item.id === 'leaf-a')!;
  expect(Math.hypot(hub.x - cluster.x, hub.y - cluster.y)).toBeLessThan(Math.hypot(leaf.x - cluster.x, leaf.y - cluster.y));
});

test('constellation placement remains stable when input order changes', () => {
  const nodes = [node('a'), node('b'), node('c')];
  const edges = [edge('a', 'b'), edge('b', 'c')];
  const first = layoutNodes(nodes, edges, 900, 560).map(({ id, x, y }) => ({ id, x, y }));
  const second = layoutNodes([...nodes].reverse(), edges, 900, 560).map(({ id, x, y }) => ({ id, x, y }));
  expect([...first].sort((a, b) => a.id.localeCompare(b.id))).toEqual([...second].sort((a, b) => a.id.localeCompare(b.id)));
});
```

- [ ] **Step 2: Run the placement tests and verify the red phase**

Run: `bun test ./src/components/graphLayout.test.ts`

Expected: FAIL because current points are not biased toward the hub and the new test expectations are not yet satisfied.

- [ ] **Step 3: Implement hub-centered radial placement**

Compute a stable angle and radial fraction from the node name hash. Put the highest-degree node(s) near the cluster center; distribute leaves around an outer radius based on cluster size. Clamp all points to the padded canvas. Do not introduce random values or a continuously running force simulation. Preserve the existing `previous` positions in `ObsidianGraph` when gateway payloads refresh.

- [ ] **Step 4: Run placement and semantic tests**

Run: `bun test ./src/components/graphLayout.test.ts ./src/components/graphSemantics.test.ts`

Expected: PASS with stable ordering, hub placement, and existing semantic checks.

- [ ] **Step 5: Commit the layout hierarchy**

```bash
git add web/src/components/graphLayout.ts web/src/components/graphLayout.test.ts
git commit -m "feat: arrange graph communities as constellations"
```

### Task 3: Apply the reference visual language to the canvas

**Files:**
- Modify: `web/src/components/ObsidianGraph.tsx:64-110,650-930,1000-1080`

**Interfaces:**
- Keep `ObsidianGraphProps` and all selection callbacks compatible with `KnowledgeView`.
- Extend `onVisibleStatsChange` to `{ loaded: number; visible: number; edges: number; renderedNodes: number; renderedEdges: number }` and update its single caller in `KnowledgeView.tsx` in the same task.

- [ ] **Step 1: Add a visual-state regression test for defaults**

Create a pure exported constant in `ObsidianGraph.tsx`:

```ts
export const DEFAULT_GRAPH_DISPLAY = {
  showLabels: true,
  showGlow: true,
  showParticles: true,
  curvedLinks: false,
  showSpaceField: true,
  showGrid: false,
  showOrbits: false,
} as const;
```

Add a Bun test in `web/src/components/graphDisplay.test.ts` asserting grid and neighbor orbits are off by default while labels, glow, and space field remain on. This test is data-only and does not require a browser.

- [ ] **Step 2: Run the new test and verify the red phase**

Run: `bun test ./src/components/graphDisplay.test.ts`

Expected: FAIL because the exported default constant does not exist and current defaults enable the grid/orbits.

- [ ] **Step 3: Implement the constellation drawing pass**

Use the existing Canvas 2D loop and make these concrete changes:

- Replace the grid-first background with the existing radial dark gradient plus the deterministic star field; draw community halos as faint radial gradients without opaque cluster fills.
- Use the node palette for ordinary relation lines at low alpha, then brighten selected/hovered/high-confidence lines. Keep provenance-missing links dashed.
- Draw every viewport-visible node at a minimum visible radius (about 2px at fit scale), scale hubs by square-root degree, and give selected/search-matched nodes a stronger halo.
- Use a single label budget: always label selected, hovered, and search matches; label hubs above the dynamic degree threshold; reveal ordinary labels only above zoom 1.15. Truncate labels at 36 characters.
- Keep particle animation disabled for reduced-motion users and for large edge sets.
- Call `prioritizeGraphEdges(..., 1800)` before drawing and report both authoritative relations and rendered edges.
- Keep hit testing on `visibleNodesRef`/`visibleEdgesRef`, and make the cursor/touch affordance clear on the canvas.

- [ ] **Step 4: Run the focused WebUI tests**

Run: `bun test ./src/components/graphDisplay.test.ts ./src/components/graphLayout.test.ts ./src/components/graphSemantics.test.ts`

Expected: PASS.

- [ ] **Step 5: Commit the canvas visual update**

```bash
git add web/src/components/ObsidianGraph.tsx web/src/components/graphDisplay.test.ts web/src/index.css
git commit -m "feat: style memory graph as constellation atlas"
```

### Task 4: Update Knowledge page counts, controls, and responsive presentation

**Files:**
- Modify: `web/src/components/KnowledgeView.tsx:165,420-450,660-690,700-715`
- Modify: `web/src/components/ObsidianGraph.tsx:995-1120`

**Interfaces:**
- `visibleGraphStats` stores `{ loaded: number; visible: number; edges: number; renderedNodes: number; renderedEdges: number }`.
- The graph footer and status card must distinguish backend totals from viewport-visible and renderer-budget counts.

- [ ] **Step 1: Add a formatting test for truthful graph counts**

Create `web/src/components/graphStats.test.ts` with:

```ts
import { expect, test } from 'bun:test';
import { formatGraphStats } from './graphStats';

test('formats authoritative and rendered counts separately', () => {
  expect(formatGraphStats({ loaded: 1000, visible: 986, edges: 7200, renderedNodes: 986, renderedEdges: 1800 }))
    .toBe('1,000 loaded · 986 visible · 7,200 relations · 1,800 rendered');
});
```

Add `web/src/components/graphStats.ts` with the typed `GraphStats` interface and `formatGraphStats` implementation.

- [ ] **Step 2: Run the stats test and verify the red phase**

Run: `bun test ./src/components/graphStats.test.ts`

Expected: FAIL before the helper is created.

- [ ] **Step 3: Wire truthful status and responsive controls**

Update `KnowledgeView` cards/footer to use `formatGraphStats`, keep backend totals visible, and change the overview helper copy from “groups records” to “shows every loaded entity; zoom reveals labels and relation detail.” Make the graph-plus-inspector grid stack at `xl` as it does today, add `min-h-11` to floating graph buttons, and ensure the status card/legend do not cover the central constellation on narrow widths.

- [ ] **Step 4: Run the focused tests and gates**

Run: `bun test ./src/components/graphStats.test.ts ./src/components/graphLayout.test.ts ./src/components/graphSemantics.test.ts`

Expected: PASS.

- [ ] **Step 5: Commit the page presentation update**

```bash
git add web/src/components/KnowledgeView.tsx web/src/components/ObsidianGraph.tsx web/src/components/graphStats.ts web/src/components/graphStats.test.ts
git commit -m "feat: clarify graph coverage and renderer counts"
```

### Task 5: Final focused verification and handoff

**Files:**
- Verify: `web/src/components/ObsidianGraph.tsx`
- Verify: `web/src/components/graphLayout.ts`
- Verify: `web/src/components/KnowledgeView.tsx`
- Verify: `web/src/components/graphLayout.test.ts`
- Verify: `web/src/components/graphDisplay.test.ts`
- Verify: `web/src/components/graphStats.test.ts`

- [ ] **Step 1: Run all WebUI tests**

Run from `web/`: `bun test`

Expected: all existing and new tests pass with zero failures.

- [ ] **Step 2: Run lint and production build**

Run from `web/`: `bun run lint`

Expected: exit 0 with no new warnings/errors.

Run from `web/`: `bun run build`

Expected: TypeScript and Vite build complete successfully. An existing large-bundle advisory is acceptable if it remains unchanged.

- [ ] **Step 3: Inspect the final diff without broad staging**

Run: `git diff --check` and `git status --short`.

Expected: no whitespace errors; unrelated dirty files remain unstaged.

- [ ] **Step 4: Perform a real-gateway smoke pass when port 8765 is available**

Open the Knowledge page and verify:

1. The overview displays the full loaded count as low-detail points, not only six representatives per community.
2. Zooming in reveals labels and relation lines; search focuses a matching entity even when it began off-screen.
3. Selecting a hub brightens its incident paths and the inspector shows real observations, confidence, provenance, and source.
4. The status strip distinguishes backend relation totals from the renderer edge budget.
5. Reduced-motion mode removes particles/settling without disabling navigation.

- [ ] **Step 5: Update the remediation checklist**

After verification, mark the constellation redesign and focused gates complete in `plan/webui-remediation-todo.md` only if the smoke pass succeeds. Do not mark a real-gateway check complete while port `8765` is unavailable.
