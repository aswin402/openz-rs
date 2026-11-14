import React, { useMemo, useState } from 'react';
import type { ToolExecution } from '../types';
import {
  ToolCall,
  mapToolExecutionToToolCallData,
  resolveInitialToolCallOpen,
  type ToolResultStyleChoice,
} from './assistant-ui/elements';
import {
  ImageGeneration,
  extractToolImagePayload,
} from './beui';
import { useUiCustomizationStore } from '../store/useUiCustomizationStore';
import { cn } from '../lib/utils';

export interface ToolExecutionCardProps {
  tool: ToolExecution;
  styleOverride?: ToolResultStyleChoice;
  className?: string;
}

export const ToolExecutionCard: React.FC<ToolExecutionCardProps> = ({
  tool,
  styleOverride,
  className,
}) => {
  const storeToolStyle = useUiCustomizationStore((s) => s.toolResultStyle);
  const imageGenStyle = useUiCustomizationStore((s) => s.imageGenStyle);
  const effectiveStyle = styleOverride ?? storeToolStyle;

  const mapped = useMemo(() => mapToolExecutionToToolCallData(tool), [tool]);
  const defaultOpen = resolveInitialToolCallOpen(effectiveStyle, mapped.failed ?? false);
  const [userOpen, setUserOpen] = useState<boolean | null>(null);
  const isOpen = userOpen ?? defaultOpen;

  const imagePayload = useMemo(
    () => extractToolImagePayload(tool.name, tool.args, tool.output ?? ''),
    [tool.name, tool.args, tool.output],
  );

  return (
    <div className={cn('flex w-full min-w-0 flex-col gap-2', className)}>
      <ToolCall
        label={mapped.label}
        activeLabel={mapped.activeLabel}
        query={mapped.query}
        request={mapped.request}
        result={mapped.result}
        running={mapped.running}
        failed={mapped.failed}
        open={isOpen}
        onOpenChange={setUserOpen}
        className="max-w-xl"
      />
      {imagePayload.isImageTool &&
        (imagePayload.src || mapped.running || mapped.failed) && (
          <ImageGeneration
            src={imagePayload.src}
            prompt={imagePayload.prompt}
            status={
              mapped.running
                ? 'generating'
                : mapped.failed
                  ? 'failed'
                  : 'completed'
            }
            variant={imageGenStyle}
          />
        )}
    </div>
  );
};
