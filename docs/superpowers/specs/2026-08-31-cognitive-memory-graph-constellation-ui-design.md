# Cognitive Memory Graph Constellation UI

## Objective

Redesign the Cognitive Memory Graph so it reads as a dense, connected constellation like the supplied references while remaining a trustworthy view of the complete gateway payload. The graph must represent every loaded entity, avoid invented/demo content, keep relationship context discoverable, and remain responsive for approximately 1,000 nodes and a substantially larger edge set.

## Design direction

Use a dark observatory canvas with a near-black radial background, restrained star points, and community-colored node systems. The visual hierarchy is:

1. High-degree or selected entities become bright, larger hubs with a subtle glow.
2. Other entities remain visible as small points, preserving the full loaded graph instead of replacing it with representative-only clusters.
3. Relationship lines stay thin and low-opacity at overview scale; selected-node neighborhoods and high-confidence relations become brighter.
4. Community halos/orbits provide the “universe” grouping from the reference images without hiding the underlying records.
5. Names appear progressively: selected and hovered nodes are always labeled, important hubs are labeled at normal zoom, and dense labels appear only after zooming in.

Colors are derived deterministically from the real entity type/community identity. No personal names, locations, sample facts, or synthetic entities are introduced by the client.

## User experience

### Canvas

- Default overview renders all loaded entities as low-detail points; it may reduce edge density through deterministic level-of-detail, but it must report loaded, represented/rendered, and visible edge counts separately.
- `All nodes` keeps every node eligible for viewport-culling and inspection.
- `Neighborhood` emphasizes the selected node, its neighbors, and the connecting relations.
- Pan, wheel/pinch zoom, node drag/pin, click selection, edge selection, cluster/community focus, fit-to-view, and reset remain available.
- A selected node receives a halo, stronger label, and highlighted incident paths. The inspector remains the authoritative source for observations, relation confidence, provenance, and source.

### Controls

- Keep compact floating controls for zoom in/out, fit, reset, and display settings.
- Keep search, node type, community, relation, fact, and connectivity filters in the Knowledge page.
- Show a small live status strip with gateway state and `loaded / represented / visible / rendered edges` counts.
- Keep settings for labels, glow, particles, curved links, grid, and orbits, but make the default constellation prioritize the graph over decorative effects.

### Responsive behavior

- Desktop uses a wide graph plus inspector layout.
- Narrow screens stack the inspector beneath the graph, preserve a minimum 44px control target, and keep the canvas height usable without horizontal overflow.
- Respect `prefers-reduced-motion` by disabling moving particles and continuous settling while retaining pan/zoom.

## Rendering and performance contract

- Cap canvas device-pixel ratio at 2 and avoid allocations inside the steady-state draw loop where practical.
- Build layout and semantic maps only when node/edge inputs change; preserve stable positions across gateway refreshes.
- Use viewport culling for hit testing and drawing. Apply deterministic edge level-of-detail at low zoom and prioritize selected, hovered, high-confidence, and high-degree edges.
- Keep the current hard safety cap for rendered edges, expose the cap in code, and make the UI count distinguish backend relations from currently rendered lines.
- Avoid unbounded force simulation. If settling is used, it must stop after a bounded number of frames and remain deterministic enough for repeatable screenshots/tests.
- Search and selection must be able to reveal/focus a matching node even when it is outside the current viewport or outside the low-zoom edge budget.

## Component boundaries

- `KnowledgeView.tsx` owns page filters, tabs, authoritative totals, and the selected-node inspector.
- `ObsidianGraph.tsx` owns canvas interaction, drawing, visual display settings, and visible/rendered statistics.
- `graphLayout.ts` owns deterministic positions, communities, viewport selection, and level-of-detail selection; helpers remain data-only for tests.
- `graphSemantics.ts` owns confidence/provenance normalization and node/edge metrics.
- `openz.ts` remains the typed boundary for gateway payloads; no graph-only demo fallback is permitted.

## Validation

Add or update data-only tests for:

- all loaded nodes remaining represented in overview mode;
- deterministic community/layout ordering;
- edge level-of-detail prioritizing selected and high-degree/high-confidence relations;
- search revealing a matching endpoint outside the current viewport;
- malformed records being ignored without dropping valid entities.

Run only the focused WebUI gates: `bun test`, `bun run lint`, and `bun run build`. Do not run full-workspace Cargo commands for this UI slice.

## Out of scope

- Changing the gateway memory schema or adding a second graph service.
- Inventing labels, entities, or relationships to make an empty graph look populated.
- Replacing the existing inspector, markdown export, or facts tab.
