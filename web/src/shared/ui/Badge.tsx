import type { FC, ReactNode } from 'react';
import { cn } from '../../lib/utils';

type BadgeTone = 'amber' | 'green' | 'red' | 'muted';

export const Badge: FC<{ children: ReactNode; tone?: BadgeTone }> = ({ children, tone = 'muted' }) => {
  const classes: Record<BadgeTone, string> = {
    amber: 'bg-amber-500/10 text-amber-400',
    green: 'bg-emerald-500/10 text-emerald-400',
    red: 'bg-red-500/10 text-red-400',
    muted: 'bg-muted/70 text-muted-foreground',
  };
  return <span className={cn('rounded-full px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider', classes[tone])}>{children}</span>;
};
