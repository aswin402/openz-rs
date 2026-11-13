import React from 'react';
import type { ToolExecution } from '../../types';
import { ToolResult } from '../../components/beui';
import { useUiCustomizationStore } from '../../store/useUiCustomizationStore';

interface ToolExecutionCardProps {
  tool: ToolExecution;
}

export const ToolExecutionCard: React.FC<ToolExecutionCardProps> = ({ tool }) => {
  const toolResultStyle = useUiCustomizationStore((s) => s.toolResultStyle);
  const diffLayout = useUiCustomizationStore((s) => s.diffLayout);
  const imageGenStyle = useUiCustomizationStore((s) => s.imageGenStyle);
  const terminalStyle = useUiCustomizationStore((s) => s.terminalStyle);
  const jsonViewerStyle = useUiCustomizationStore((s) => s.jsonViewerStyle);
  const subagentCardStyle = useUiCustomizationStore((s) => s.subagentCardStyle);

  return (
    <ToolResult
      tool={tool}
      variant={toolResultStyle}
      diffLayout={diffLayout}
      imageVariant={imageGenStyle}
      terminalVariant={terminalStyle}
      jsonVariant={jsonViewerStyle}
      subagentVariant={subagentCardStyle}
    />
  );
};
