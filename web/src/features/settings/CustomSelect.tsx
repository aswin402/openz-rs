import React, { useEffect, useRef, useState } from 'react';
import { ChevronDown, ChevronUp } from 'lucide-react';
import type { SelectGroup, SelectOption } from './settingsSchema';

export interface CustomSelectProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  options?: SelectOption[];
  groups?: SelectGroup[];
}

export const CustomSelect: React.FC<CustomSelectProps> = ({ label, value, onChange, options, groups }) => {
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
      <label className="mb-1 block font-medium text-foreground">{label}</label>
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        className="flex w-full items-center justify-between rounded-lg border border-border bg-muted/40 p-2.5 text-xs text-foreground hover:bg-muted/60 transition-colors focus:outline-none focus:ring-1 focus:ring-amber-500"
      >
        <span className="truncate">{displayValue}</span>
        <ChevronDown
          className="h-4 w-4 shrink-0 text-amber-500 transition-transform duration-200"
          style={{ transform: isOpen ? 'rotate(180deg)' : 'rotate(0)' }}
        />
      </button>

      {isOpen && (
        <div className="absolute left-0 right-0 z-50 mt-1 max-h-60 overflow-y-auto rounded-lg border border-border bg-card p-1.5 shadow-xl animate-in fade-in slide-in-from-top-1 duration-150 scrollbar-thin">
          {options && (
            <div className="space-y-0.5">
              {options.map((option) => (
                <button
                  key={option.value}
                  type="button"
                  onClick={() => {
                    onChange(option.value);
                    setIsOpen(false);
                  }}
                  className={`flex w-full items-center rounded-md px-2.5 py-2 text-left text-xs transition-colors hover:bg-amber-500 hover:text-white ${
                    value === option.value ? 'bg-amber-500/10 text-amber-500 font-semibold' : 'text-foreground'
                  }`}
                >
                  {option.label}
                </button>
              ))}
            </div>
          )}

          {groups && (
            <div className="space-y-3">
              {groups.map((group) => (
                <div key={group.label}>
                  <div className="px-2.5 py-1 text-[10px] font-bold uppercase tracking-wider text-muted-foreground/60 select-none">
                    {group.label}
                  </div>
                  <div className="mt-1 space-y-0.5 pl-1 border-l border-border/40 ml-1">
                    {group.options.map((option) => (
                      <button
                        key={option.value}
                        type="button"
                        onClick={() => {
                          onChange(option.value);
                          setIsOpen(false);
                        }}
                        className={`flex w-full items-center rounded-md px-2.5 py-1.5 text-left text-xs transition-colors hover:bg-amber-500 hover:text-white ${
                          value === option.value ? 'bg-amber-500/10 text-amber-500 font-semibold' : 'text-foreground'
                        }`}
                      >
                        {option.label}
                      </button>
                    ))}
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

export const NumberInput: React.FC<NumberInputProps> = ({ label, value, onChange, min = 1, max, step = 1 }) => {
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
      <label className="mb-1 block font-medium text-foreground">{label}</label>
      <div className="relative flex items-center">
        <input
          type="number"
          min={min}
          max={max}
          step={step}
          value={value}
          onChange={(event) => onChange(Number(event.target.value))}
          className="w-full rounded-lg border border-border bg-muted/40 p-2.5 pr-8 text-xs text-foreground font-mono focus:outline-none focus:ring-1 focus:ring-amber-500 [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
        />
        <div className="absolute right-1.5 flex flex-col gap-0.5 select-none">
          <button
            type="button"
            onClick={handleIncrement}
            className="flex h-3.5 w-4 items-center justify-center rounded-sm bg-muted/50 hover:bg-amber-500 hover:text-white text-muted-foreground/80 transition-colors"
          >
            <ChevronUp className="h-2 w-2" strokeWidth={3} />
          </button>
          <button
            type="button"
            onClick={handleDecrement}
            className="flex h-3.5 w-4 items-center justify-center rounded-sm bg-muted/50 hover:bg-amber-500 hover:text-white text-muted-foreground/80 transition-colors"
          >
            <ChevronDown className="h-2 w-2" strokeWidth={3} />
          </button>
        </div>
      </div>
    </div>
  );
};
