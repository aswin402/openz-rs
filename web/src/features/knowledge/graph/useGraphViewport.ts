import type { GraphCluster } from '../graphLayout';
import type { RenderNode, Viewport } from './graphCanvas';

export interface GraphTransform {
  x: number;
  y: number;
  k: number;
}

interface CanvasSize {
  width: number;
  height: number;
}

function canvasSize(canvas: HTMLCanvasElement | null, fallbackWidth = 800, fallbackHeight = 560): CanvasSize {
  return {
    width: canvas?.clientWidth || fallbackWidth,
    height: canvas?.clientHeight || fallbackHeight,
  };
}

export function viewportForCanvas(canvas: HTMLCanvasElement | null, transform: GraphTransform, padding = 80): Viewport {
  const { width, height } = canvasSize(canvas);
  return {
    left: -transform.x / transform.k - padding,
    top: -transform.y / transform.k - padding,
    right: (width - transform.x) / transform.k + padding,
    bottom: (height - transform.y) / transform.k + padding,
  };
}

export function graphPointFromScreen(
  canvas: HTMLCanvasElement | null,
  transform: GraphTransform,
  screenX: number,
  screenY: number,
): { x: number; y: number } {
  if (!canvas) return { x: 0, y: 0 };
  const rect = canvas.getBoundingClientRect();
  return {
    x: (screenX - rect.left - transform.x) / transform.k,
    y: (screenY - rect.top - transform.y) / transform.k,
  };
}

export function zoomTransformAtPoint(
  transform: GraphTransform,
  pointX: number,
  pointY: number,
  factor: number,
  minimum = 0.18,
  maximum = 5,
): GraphTransform {
  const nextK = Math.min(maximum, Math.max(minimum, transform.k * factor));
  return {
    x: pointX - (pointX - transform.x) * (nextK / transform.k),
    y: pointY - (pointY - transform.y) * (nextK / transform.k),
    k: nextK,
  };
}

export function fitTransform(
  canvas: HTMLCanvasElement | null,
  source: Array<{ x: number; y: number; radius: number }>,
): GraphTransform | null {
  if (!canvas || source.length === 0) return null;
  const { width, height } = canvasSize(canvas);
  const minX = Math.min(...source.map((item) => item.x - item.radius));
  const maxX = Math.max(...source.map((item) => item.x + item.radius));
  const minY = Math.min(...source.map((item) => item.y - item.radius));
  const maxY = Math.max(...source.map((item) => item.y + item.radius));
  const graphWidth = Math.max(180, maxX - minX + 120);
  const graphHeight = Math.max(180, maxY - minY + 120);
  const scale = Math.min(1.55, Math.max(0.35, Math.min(width / graphWidth, height / graphHeight) * 0.9));
  return {
    x: width / 2 - ((minX + maxX) / 2) * scale,
    y: height / 2 - ((minY + maxY) / 2) * scale,
    k: scale,
  };
}

export function focusNodeTransform(
  canvas: HTMLCanvasElement | null,
  node: Pick<RenderNode, 'x' | 'y'>,
  currentZoom: number,
): GraphTransform | null {
  if (!canvas) return null;
  const { width, height } = canvasSize(canvas);
  const scale = Math.min(2.2, Math.max(currentZoom, 1.18));
  return {
    x: width / 2 - node.x * scale,
    y: height / 2 - node.y * scale,
    k: scale,
  };
}

export function focusClusterTransform(canvas: HTMLCanvasElement | null, cluster: GraphCluster): GraphTransform | null {
  if (!canvas) return null;
  const { width, height } = canvasSize(canvas);
  const scale = Math.min(2.25, Math.max(1.15, Math.min(width, height) / (cluster.radius * 3)));
  return {
    x: width / 2 - cluster.x * scale,
    y: height / 2 - cluster.y * scale,
    k: scale,
  };
}
