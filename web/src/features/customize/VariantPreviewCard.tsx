import React from 'react';
import { Check } from 'lucide-react';
import { Badge } from '../../components/ui/badge';
import { Button } from '../../components/ui/button';
import { cn } from '../../lib/utils';

export interface VariantPreviewCardProps {
  title: string;
  description: string;
  selected: boolean;
  isDefault?: boolean;
  onSelect: () => void;
  children: React.ReactNode;
}

export const VariantPreviewCard: React.FC<VariantPreviewCardProps> = ({
  title,
  description,
  selected,
  isDefault = false,
  onSelect,
  children,
}) => {
  return (
    <div
      className={cn(
        'flex flex-col overflow-hidden rounded-xl border bg-card transition-colors',
        selected ? 'border-primary ring-1 ring-primary/35' : 'border-border/80 hover:border-border',
      )}
    >
      <div className="flex items-center justify-between gap-2 border-b border-border/60 bg-muted/25 px-4 py-2.5">
        <div className="flex items-center gap-2">
          <span className="text-xs font-semibold text-foreground">{title}</span>
          {isDefault && (
            <Badge variant="outline" className="px-1.5 py-0 text-[10px]">
              Default
            </Badge>
          )}
        </div>
        {selected && (
          <Badge variant="success" className="gap-1 px-2 py-0 text-[10px]">
            <Check className="size-2.5" />
            <span>Active</span>
          </Badge>
        )}
      </div>

      <div className="flex min-h-[175px] flex-1 items-center justify-center bg-background/90 p-5">
        <div className="w-full max-w-2xl">{children}</div>
      </div>

      <div className="flex items-center justify-between gap-3 border-t border-border/60 bg-card px-4 py-3">
        <p className="min-w-0 flex-1 text-xs text-muted-foreground">{description}</p>
        <Button
          type="button"
          size="sm"
          variant={selected ? 'secondary' : 'default'}
          onClick={onSelect}
          className={cn(
            'shrink-0 min-w-[96px]',
            selected && 'border border-primary/45 bg-primary/15 text-primary hover:bg-primary/20',
          )}
        >
          {selected ? (
            <>
              <Check className="size-3.5" />
              <span>Selected</span>
            </>
          ) : (
            <span>Select</span>
          )}
        </Button>
      </div>
    </div>
  );
};
