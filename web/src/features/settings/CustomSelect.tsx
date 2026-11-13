import React, { useEffect, useRef, useState } from 'react';
import { Check, ChevronDown, ChevronUp } from 'lucide-react';
import type { SelectGroup, SelectOption } from './settingsSchema';
import { cn } from '../../lib/utils';

export interface CustomSelectProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  options?: SelectOption[];
  groups?: SelectGroup[];
}

export const CustomSelect: React.FC<CustomSelectProps> = ({
  label,
  value,
  onChange,
  options,
  groups,
}) => {
  const [isOpen, setIsOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
        setIsOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  let displayValue = value;
  if (options) {
    const matched = options.find((option) => option.value === value);
    if (matched) displayValue = matched.label;
  } else if (groups) {
    for (const group of groups) {
      const matched = group.options.find((option) => option.value === value);
      if (matched) {
        displayValue = matched.label;
        break;
      }
    }
  }

  return (
    <div ref={containerRef} className="relative w-full">
      <label className="mb-1.5 block text-xs font-medium text-muted-foreground select-none">
        {label}
      </label>
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        className={cn(
          'flex w-full items-center justify-between rounded-xl border bg-background/70 px-3 py-2.5 text-xs text-foreground transition-all hover:bg-muted/50 focus:outline-none',
          isOpen
            ? 'border-primary ring-2 ring-primary/20'
            : 'border-border/70 hover:border-border',
        )}
      >
        <span className="truncate font-medium">{displayValue}</span>
        <ChevronDown
          className="h-4 w-4 shrink-0 text-primary transition-transform duration-200"
          style={{ transform: isOpen ? 'rotate(180deg)' : 'rotate(0)' }}
        />
      </button>

      {isOpen && (
        <div className="t-popover absolute left-0 right-0 z-50 mt-1.5 max-h-64 overflow-y-auto rounded-xl border border-border bg-card p-1.5 shadow-2xl scrollbar-thin">
          {options && (
            <div className="space-y-0.5">
              {options.map((option) => {
                const selected = value === option.value;
                return (
                  <button
                    key={option.value}
                    type="button"
                    onClick={() => {
                      onChange(option.value);
                      setIsOpen(false);
                    }}
                    className={cn(
                      'flex w-full items-center justify-between rounded-lg px-2.5 py-2 text-left text-xs transition-colors',
                      selected
                        ? 'bg-primary/15 font-semibold text-primary'
                        : 'text-foreground hover:bg-muted/70',
                    )}
                  >
                    <span className="truncate">{option.label}</span>
                    {selected && <Check className="h-3.5 w-3.5 shrink-0 text-primary" />}
                  </button>
                );
              })}
            </div>
          )}

          {groups && (
            <div className="space-y-3">
              {groups.map((group) => (
                <div key={group.label}>
                  <div className="select-none px-2.5 py-1 text-[10px] font-bold uppercase tracking-wider text-muted-foreground/75">
                    {group.label}
                  </div>
                  <div className="ml-1.5 mt-1 space-y-0.5 border-l border-border/50 pl-1.5">
                    {group.options.map((option) => {
                      const selected = value === option.value;
                      return (
                        <button
                          key={option.value}
                          type="button"
                          onClick={() => {
                            onChange(option.value);
                            setIsOpen(false);
                          }}
                          className={cn(
                            'flex w-full items-center justify-between rounded-lg px-2.5 py-1.5 text-left text-xs transition-colors',
                            selected
                              ? 'bg-primary/15 font-semibold text-primary'
                              : 'text-foreground hover:bg-muted/70',
                          )}
                        >
                          <span className="truncate font-mono text-[11px]">{option.label}</span>
                          {selected && <Check className="h-3.5 w-3.5 shrink-0 text-primary" />}
                        </button>
                      );
                    })}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
};

export interface NumberInputProps {
  label: string;
  value: number;
  onChange: (value: number) => void;
  min?: number;
  max?: number;
  step?: number;
}

export const NumberInput: React.FC<NumberInputProps> = ({
  label,
  value,
  onChange,
  min = 1,
  max,
  step = 1,
}) => {
  const handleIncrement = () => {
    const newValue = value + step;
    if (max !== undefined && newValue > max) return;
    onChange(Number(newValue.toFixed(2)));
  };

  const handleDecrement = () => {
    const newValue = value - step;
    if (min !== undefined && newValue < min) return;
    onChange(Number(newValue.toFixed(2)));
  };

  return (
    <div>
      <label className="mb-1.5 block text-xs font-medium text-muted-foreground select-none">
        {label}
      </label>
      <div className="relative flex items-center">
        <input
          type="number"
          min={min}
          max={max}
          step={step}
          value={value}
          onChange={(event) => onChange(Number(event.target.value))}
          className="w-full rounded-xl border border-border/70 bg-background/70 px-3 py-2.5 pr-8 font-mono text-xs text-foreground transition-all hover:border-border focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20 [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
        />
        <div className="absolute right-1.5 flex select-none flex-col gap-0.5">
          <button
            type="button"
            onClick={handleIncrement}
            className="flex h-3.5 w-4 items-center justify-center rounded-sm bg-muted/60 text-muted-foreground transition-colors hover:bg-primary hover:text-primary-foreground"
            aria-label={`Increase ${label}`}
          >
            <ChevronUp className="h-2.5 w-2.5" strokeWidth={2.5} />
          </button>
          <button
            type="button"
            onClick={handleDecrement}
            className="flex h-3.5 w-4 items-center justify-center rounded-sm bg-muted/60 text-muted-foreground transition-colors hover:bg-primary hover:text-primary-foreground"
            aria-label={`Decrease ${label}`}
          >
            <ChevronDown className="h-2.5 w-2.5" strokeWidth={2.5} />
          </button>
        </div>
      </div>
    </div>
  );
};
