import type { ComponentType, FC, ReactNode } from 'react';
import { cn } from '../../lib/utils';

export const Panel: FC<{
  title: string;
  icon: ComponentType<{ className?: string }>;
  children: ReactNode;
}> = ({ title, icon: Icon, children }) => (
  <section className="rounded-2xl border border-border/60 bg-card/45 p-5 shadow-sm backdrop-blur-md">
    <div className="mb-4 flex items-center gap-2 border-b border-border/40 pb-2">
      <Icon className="h-4 w-4 text-amber-500" />
      <h2 className="text-sm font-bold text-foreground">{title}</h2>
    </div>
    {children}
  </section>
);

export const KeyValue: FC<{ label: string; value: string; mono?: boolean; compact?: boolean }> = ({ label, value, mono, compact }) => (
  <div className={cn('rounded-lg border border-border/30 bg-background/30 px-3 py-2', compact && 'border-0 bg-transparent px-0 py-0')}>
    <div className="text-[10px] font-bold uppercase tracking-wider text-muted-foreground/70">{label}</div>
    <div className={cn('mt-0.5 break-words text-xs text-foreground', mono && 'font-mono')}>{value}</div>
  </div>
);
