# Cognitive Memory Graph Redesign

## Objective

Make the Cognitive Memory Graph a useful, trustworthy exploration surface for the complete OpenZ memory graph. The default view must be readable for roughly 1,000 entities while keeping every backend record searchable, selectable, and inspectable. The graph must never invent demo data such as personal names or locations that are not returned by the gateway.

## User experience

The page follows an overview → explore → inspect flow:

1. The header presents live entity, relation, and fact counts, database availability, last synchronization time, and an explicit loaded-record status.
2. A compact toolbar provides full-text search over node names, types, observations, and relations; entity-type filtering; overview/all-node/neighborhood modes; and reset-view controls.
3. The graph opens as a readable cluster overview. All records remain in the client data model, but low-zoom rendering uses cluster representatives and suppresses low-value labels. Zooming, searching, or selecting a cluster progressively reveals its member nodes.
4. A detail inspector shows the selected entity’s type, observations, degree, connected entities, and relation types. It can focus the graph on the selected entity’s neighborhood.
5. Facts stay in the facts surface rather than being converted into synthetic graph nodes.

The visual language should fit the existing dark OpenZ interface: clear hierarchy, restrained accent color, compact status chips, and responsive behavior. On small screens the inspector becomes a bottom sheet or stacked panel and controls remain touch-sized.

## Data contract

- Graph content comes only from `cognitive_memory` WebSocket payloads (`nodes`, `edges`, `facts`, `stats`, and runtime paths).
- No UI seed arrays, personal demo entities, location examples, fabricated facts, or fallback graph records are permitted.
- Counts displayed as loaded must reflect the actual payload, including deduplicated graph identities where the renderer uses a name key.
- A missing or unavailable database is represented as an explicit status, not as example content.

## Rendering and performance

- Keep the existing canvas renderer and React component boundary; do not add a new rendering dependency for this slice.
- Perform layout only when the data set, viewport, or layout mode changes. Use deterministic initial positions derived from a stable node identity so refreshes do not reshuffle the graph.
- Build connected components or lightweight type/community clusters once per data update. At overview zoom, render cluster hulls/representatives and a bounded sample of high-degree members; at higher zoom or during search, render individual nodes in the visible viewport.
- Cull offscreen nodes and edges before drawing. Batch edge strokes and reduce particle work to visible/selected neighborhoods instead of animating every edge.
- Never call React state setters from the animation loop. Store simulation state in refs and publish only meaningful interaction changes (hover, selection, mode, search).
- Draw labels only for selected, hovered, search-matching, or high-degree nodes. Provide a loaded-count indicator so suppression is not mistaken for missing data.
- Preserve the full node/edge arrays for search, filtering, export, and inspection even when the canvas uses level-of-detail rendering.

## Component changes

### `KnowledgeView`

- Add the live status header and graph mode controls.
- Keep filters and counts derived from the current payload; distinguish filtered counts from backend totals.
- Pass the selected node and focus callbacks to the graph/inspector.
- Keep markdown/JSON export based on the complete filtered dataset.

### `ObsidianGraph`

- Replace the current “load every node into the simulation immediately” path with deterministic layout plus level-of-detail rendering.
- Add cluster overview, all-node, and neighborhood modes without dropping nodes from the source arrays.
- Add stable node IDs/positions, viewport culling, selected-neighborhood emphasis, and a compact minimap or cluster indicator if it improves orientation without adding render cost.
- Ensure legend entries are generated from actual backend types.

### New focused graph helpers (if needed)

- Pure functions for stable hashing/positions, connected-component or cluster assignment, viewport culling, and visible-node selection.
- Keep helpers data-only so they can be tested without a browser or canvas.

## Empty/error states

- Empty graph: explain that OpenZ has no persisted entities yet and show the live database paths.
- Partial graph: show the number of received nodes/edges and a warning when a database query is unavailable; never substitute example records.
- Search with no match: preserve the overview and display a clear no-match state with a reset action.

## Verification

- `bun run lint` from `web/`.
- `bun run build` from `web/`.
- `git diff --check`.
- Add/adjust pure helper tests where practical for deterministic positions, cluster membership, visible-node culling, and no synthetic records.
- Use only scoped Rust checks if backend changes become necessary; do not run whole-workspace Cargo commands.

## Out of scope

- Replacing canvas with WebGL.
- Changing the gateway memory schema or adding a separate graph service.
- Redesigning unrelated WebUI pages.
