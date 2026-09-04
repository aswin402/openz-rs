import React from 'react';
import { getNodeColor } from './graphCanvas';

export const GraphLegend: React.FC<{ types: string[] }> = ({ types }) => {
  if (types.length === 0) return null;

  return (
    <div className="absolute bottom-3 right-3 z-10 hidden max-h-32 max-w-64 overflow-y-auto rounded-xl border border-white/10 bg-slate-950/88 p-2.5 text-[10px] shadow-lg backdrop-blur-md lg:block">
      <div className="mb-1.5 border-b border-white/10 pb-1 font-semibold text-slate-300">Entity types</div>
      <div className="grid grid-cols-2 gap-x-3 gap-y-1">
        {types.map((type) => (
          <div key={type} className="flex min-w-0 items-center gap-1.5 text-slate-500">
            <span className="h-2 w-2 shrink-0 rounded-full" style={{ backgroundColor: getNodeColor(type).main }} />
            <span className="truncate">{type}</span>
          </div>
        ))}
      </div>
    </div>
  );
};
