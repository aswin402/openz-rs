import React from 'react';
import {
  Focus,
  Layers3,
  Maximize2,
  MousePointer2,
  RotateCcw,
  Search,
  Sliders,
  X,
  ZoomIn,
  ZoomOut,
} from 'lucide-react';
import { cn } from '../../../lib/utils';
import type { GraphMode } from '../graphLayout';
import type { GraphDisplayState } from './graphCanvas';

interface GraphControlsProps {
  activeMode: GraphMode;
  activeSelectionName: string | null;
  searchQuery?: string;
  localSearch: string;
  onLocalSearchChange: (value: string) => void;
  onModeChange: (mode: GraphMode) => void;
  showSettings: boolean;
  onToggleSettings: () => void;
  onCloseSettings: () => void;
  display: GraphDisplayState;
  onDisplayChange: (key: keyof GraphDisplayState, value: boolean) => void;
  onZoom: (factor: number) => void;
  onFit: () => void;
  onReset: () => void;
  loadedNodes: number;
  loadedEdges: number;
  modeLabel: string;
}

const DISPLAY_OPTIONS: Array<readonly [keyof GraphDisplayState, string]> = [
  ['showLabels', 'Entity labels'],
  ['showGlow', 'Node glow'],
  ['showParticles', 'Relation pulses'],
  ['curvedLinks', 'Curved relations'],
  ['showSpaceField', 'Space field'],
  ['showGrid', 'Constellation grid'],
  ['showOrbits', 'Neighbor orbits'],
];

export const GraphControls: React.FC<GraphControlsProps> = ({
  activeMode,
  activeSelectionName,
  searchQuery,
  localSearch,
  onLocalSearchChange,
  onModeChange,
  showSettings,
  onToggleSettings,
  onCloseSettings,
  display,
  onDisplayChange,
  onZoom,
  onFit,
  onReset,
  loadedNodes,
  loadedEdges,
  modeLabel,
}) => (
  <>
    <div className="absolute left-3 top-3 z-10 flex max-w-[calc(100%-1.5rem)] flex-wrap items-center gap-2">
      {searchQuery === undefined && (
        <div className="flex items-center rounded-xl border border-white/10 bg-slate-950/88 px-2.5 py-1.5 shadow-lg backdrop-blur-md focus-within:border-amber-500/50">
          <Search className="mr-1.5 h-3.5 w-3.5 shrink-0 text-slate-500" />
          <input
            value={localSearch}
            onChange={(event) => onLocalSearchChange(event.target.value)}
            placeholder="Find an entity or relation"
            className="w-40 bg-transparent text-xs text-slate-200 outline-none placeholder:text-slate-600 sm:w-52"
          />
          {localSearch && (
            <button type="button" onClick={() => onLocalSearchChange('')} className="ml-1 rounded p-0.5 text-slate-500 hover:text-slate-200" aria-label="Clear graph search">
              <X className="h-3 w-3" />
            </button>
          )}
        </div>
      )}
      <div className="flex items-center gap-1 rounded-xl border border-white/10 bg-slate-950/88 p-1 shadow-lg backdrop-blur-md">
        {([
          ['overview', 'Overview', Layers3],
          ['all', 'All nodes', MousePointer2],
          ['neighborhood', 'Neighborhood', Focus],
        ] as const).map(([value, label, Icon]) => (
          <button
            key={value}
            type="button"
            onClick={() => onModeChange(value)}
            disabled={value === 'neighborhood' && !activeSelectionName}
            aria-pressed={activeMode === value}
            className={cn(
              'flex min-h-8 items-center gap-1.5 rounded-lg px-2.5 text-[10px] font-semibold transition',
              activeMode === value ? 'bg-amber-500/15 text-amber-300' : 'text-slate-500 hover:bg-white/5 hover:text-slate-200',
              value === 'neighborhood' && !activeSelectionName && 'cursor-not-allowed opacity-40',
            )}
            title={value === 'neighborhood' && !activeSelectionName ? 'Select an entity first' : label}
          >
            <Icon className="h-3 w-3" />
            <span className="hidden sm:inline">{label}</span>
          </button>
        ))}
      </div>
      <button
        type="button"
        onClick={onToggleSettings}
        className={cn('flex min-h-11 min-w-11 items-center justify-center rounded-xl border border-white/10 bg-slate-950/88 text-slate-500 shadow-lg backdrop-blur-md transition hover:text-slate-200 sm:h-9 sm:w-9 sm:min-h-0 sm:min-w-0', showSettings && 'border-amber-500/40 bg-amber-500/10 text-amber-300')}
        aria-label="Graph display settings"
        aria-pressed={showSettings}
      >
        <Sliders className="h-3.5 w-3.5" />
      </button>
    </div>

    <div className="absolute right-3 top-3 z-10 flex items-start gap-2">
      <div className="hidden rounded-xl border border-white/10 bg-slate-950/88 px-3 py-2 text-[10px] shadow-lg backdrop-blur-md lg:block">
        <div className="flex items-center gap-2 font-semibold text-slate-300">
          <span className="h-2 w-2 rounded-full bg-emerald-400 shadow-[0_0_10px_rgba(52,211,153,0.8)]" />
          {loadedNodes} records loaded
        </div>
        <div className="mt-1 text-right font-mono text-[9px] text-slate-600">{modeLabel} · {loadedEdges} relations</div>
      </div>
      <div className="flex flex-col gap-1 rounded-xl border border-white/10 bg-slate-950/88 p-1 shadow-lg backdrop-blur-md">
        <button type="button" onClick={() => onZoom(1.2)} className="flex min-h-11 min-w-11 items-center justify-center rounded-lg text-slate-500 transition hover:bg-white/5 hover:text-slate-200 sm:h-8 sm:w-8 sm:min-h-0 sm:min-w-0" title="Zoom in" aria-label="Zoom in"><ZoomIn className="h-3.5 w-3.5" /></button>
        <button type="button" onClick={() => onZoom(0.83)} className="flex min-h-11 min-w-11 items-center justify-center rounded-lg text-slate-500 transition hover:bg-white/5 hover:text-slate-200 sm:h-8 sm:w-8 sm:min-h-0 sm:min-w-0" title="Zoom out" aria-label="Zoom out"><ZoomOut className="h-3.5 w-3.5" /></button>
        <button type="button" onClick={onFit} className="flex min-h-11 min-w-11 items-center justify-center rounded-lg text-slate-500 transition hover:bg-white/5 hover:text-slate-200 sm:h-8 sm:w-8 sm:min-h-0 sm:min-w-0" title="Fit graph" aria-label="Fit graph"><Maximize2 className="h-3.5 w-3.5" /></button>
        <button type="button" onClick={onReset} className="flex min-h-11 min-w-11 items-center justify-center rounded-lg text-slate-500 transition hover:bg-white/5 hover:text-slate-200 sm:h-8 sm:w-8 sm:min-h-0 sm:min-w-0" title="Reset layout" aria-label="Reset layout"><RotateCcw className="h-3.5 w-3.5" /></button>
      </div>
    </div>

    {showSettings && (
      <div className="absolute left-3 top-16 z-20 w-72 max-w-[calc(100%-1.5rem)] rounded-2xl border border-white/10 bg-slate-950/96 p-4 text-xs shadow-2xl backdrop-blur-md">
        <div className="mb-3 flex items-center justify-between border-b border-white/10 pb-2 font-semibold text-slate-200">
          <span>Graph display</span>
          <button type="button" onClick={onCloseSettings} className="rounded p-1 text-slate-500 hover:text-slate-200" aria-label="Close graph settings"><X className="h-3.5 w-3.5" /></button>
        </div>
        <div className="space-y-2 text-[11px] text-slate-400">
          {DISPLAY_OPTIONS.map(([key, label]) => (
            <label key={key} className="flex min-h-9 cursor-pointer items-center justify-between rounded-lg px-2 hover:bg-white/5">
              <span>{label}</span>
              <input
                type="checkbox"
                checked={display[key]}
                onChange={(event) => onDisplayChange(key, event.target.checked)}
                className="h-4 w-4 accent-amber-500"
              />
            </label>
          ))}
        </div>
        <div className="mt-3 border-t border-white/10 pt-3 text-[10px] leading-relaxed text-slate-600">
          Overview shows every loaded entity. Zoom in to reveal labels and relation detail.
        </div>
      </div>
    )}
  </>
);
