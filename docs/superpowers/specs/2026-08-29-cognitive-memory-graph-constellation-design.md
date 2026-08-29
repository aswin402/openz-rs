# Cognitive Memory Graph Constellation UI

## Objective

Redesign the Cognitive Memory Graph into a live, Obsidian-inspired constellation atlas. The graph should feel like a connected universe while remaining readable, deterministic, searchable, and useful for inspecting real OpenZ memory records. The redesign includes Graphify-inspired graph semantics: degree-aware importance, community/type coloring, and relation provenance or confidence when the backend records it.

## Reference inspiration

Graphify's public graph workflow emphasizes a clickable full graph, search and filtering, community visualization, degree/importance, and explaining whether a connection is extracted or inferred: <https://github.com/Graphify-Labs/graphify>.

These ideas are interaction inspiration only. OpenZ must continue to render its own persisted cognitive-memory data and must not import Graphify records or fabricate metadata.

## Visual direction

The canvas is a deep-space atlas:

- Use a near-black blue background with a subtle deterministic star-field/grid texture.
- Represent connected or detected communities as constellation regions with soft nebula halos and labels/counts.
- Derive node size from real graph degree, with a cap. High-degree entities look like brighter, larger “suns”; regular entities look like smaller stars.
- Draw relations as luminous paths. Hovered or selected paths brighten; unrelated paths fade.
- Give the selected entity a central halo and direct neighbors orbit-like rings.
- Keep all graph records loaded while changing only visual detail across zoom levels: constellation regions, representative stars, then full entities and labels.

The visual metaphor must never introduce fake entities, locations, names, observations, or relationships.

## Interaction and information architecture

### Canvas header

The upper-left area shows the graph title, live entity/relation counts, connection status, and search. The upper-right area provides:

- Constellations mode (readable overview)
- All nodes mode (complete graph exploration)
- Neighborhood mode (selected entity and connected neighbors)
- Fit, reset, zoom, and display settings

### Filters

A compact responsive filter rail provides entity type/community filters, a most-connected toggle, relation-type filtering, and a confidence/provenance filter. Filters operate on live graph data and preserve the complete unfiltered dataset in memory.

### Inspector

The right-side inspector displays the selected entity's name, type, real degree, observations, neighbors, and relation rows. Each relation row can show its type, direction, confidence/provenance (`extracted`, `inferred`, or another persisted backend value), and source/context when available. Missing fields display `Not recorded`; the UI must never invent provenance.

Selecting a relation focuses both endpoints and opens its detail. Searching can reveal matching nodes and relations even if they are outside the current viewport. A small legend explains node size, color, halo, and relation styling.

## Data and rendering architecture

1. The cognitive-memory endpoint remains the source of truth. Extend its response only with values that are persisted or safely derived: degree, community/cluster key, relation direction, and optional provenance/source metadata.
2. The page keeps complete node and edge arrays. The renderer derives a bounded visible slice from mode, zoom, viewport, search, and selection.
3. Layout positions are generated from stable identifiers and graph structure so refreshes do not reshuffle the universe.
4. Canvas renders stars, halos, links, and the background. DOM is reserved for controls, labels, accessibility text, and the inspector.
5. Glow, particles, and labels are adaptive. Dense views and active pan/zoom reduce expensive effects; viewport culling prevents off-screen draw work.

## States and failure behavior

- Loading: show a calm skeleton/space state without synthetic graph records.
- Empty: explain that no persisted graph records are available and provide a refresh action.
- Stale/disconnected: show the last loaded counts with a clear gateway connection state.
- Malformed records: skip invalid nodes/edges safely and surface a non-blocking warning count.
- Missing optional semantics: render `Not recorded` and retain normal relation navigation.

## Correctness and performance checks

Add focused coverage for:

- provenance fallback and optional field handling;
- degree-based sizing and cluster coloring;
- deterministic layout stability;
- retention of the complete dataset while rendering a bounded slice;
- search, relation, and selection reveal behavior;
- 1,000-node rendering bounds and effect throttling.

Verification is intentionally scoped to the repository's low-resource workflow: `bun run lint`, `bun run build`, and focused graph smoke checks. Do not run full Cargo check/build/test commands for this UI task.

## Out of scope

- Replacing the existing graph database or memory schema.
- Importing Graphify output or adding a second graph backend.
- Fabricating source/provenance values when OpenZ does not store them.
- Adding unrelated WebUI page redesigns in this iteration.
