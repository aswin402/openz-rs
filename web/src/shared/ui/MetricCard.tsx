import type { ComponentType, FC, ReactNode } from 'react';
import { cn } from '../../lib/utils';

export interface MetricCardProps {
  label: string;
  value: string;
  detail?: string;
  icon: ComponentType<{ className?: string }>;
  accent?: string;
  onClick?: () => void;
  children?: ReactNode;
}

export const MetricCard: FC<MetricCardProps> = ({ label, value, detail, icon: Icon, accent = 'text-amber-500', onClick, children }) => {
  const content = (
    <>
      <div className="flex items-center justify-between">
        <span className="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">{label}</span>
        <Icon className={cn('h-4 w-4', accent)} />
      </div>
      <div className="mt-1.5 truncate text-xl font-extrabold tracking-tight text-foreground">{value}</div>
      {detail && <div className="mt-0.5 truncate text-[11px] text-muted-foreground">{detail}</div>}
      {children}
    </>
  );
  const className = cn(
    'w-full rounded-xl border border-border/60 bg-card/60 p-4 text-left shadow-sm',
    onClick && 'cursor-pointer transition hover:border-amber-500/40 hover:bg-card',
  );

  return onClick ? <button type="button" onClick={onClick} className={className}>{content}</button> : <div className={className}>{content}</div>;
};
