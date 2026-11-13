import type { FC, ReactNode } from 'react';
import { cn } from '../../lib/utils';

type BadgeTone = 'amber' | 'green' | 'red' | 'muted';

export const Badge: FC<{ children: ReactNode; tone?: BadgeTone }> = ({ children, tone = 'muted' }) => {
  const classes: Record<BadgeTone, string> = {
    amber: 'bg-primary/12 text-primary',
    green: 'bg-success/12 text-success',
    red: 'bg-destructive/12 text-destructive',
    muted: 'bg-muted/70 text-muted-foreground',
  };
  return <span className={cn('rounded-full px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider', classes[tone])}>{children}</span>;
};
