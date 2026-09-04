export interface GraphStats {
  loaded: number;
  visible: number;
  edges: number;
  renderedNodes: number;
  renderedEdges: number;
}

const numberFormatter = new Intl.NumberFormat('en-US');

export function formatGraphStats(stats: GraphStats): string {
  return `${numberFormatter.format(stats.loaded)} loaded · ${numberFormatter.format(stats.visible)} visible · ${numberFormatter.format(stats.edges)} relations · ${numberFormatter.format(stats.renderedEdges)} rendered`;
}
