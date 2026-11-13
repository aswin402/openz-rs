import type { OrchestrationRunState } from '../../types';
import {
  BeuiSubagentCard,
  orchestrationRunToSubagentProps,
} from '../../components/beui';
import { useUiCustomizationStore } from '../../store/useUiCustomizationStore';

export function OrchestrationRunPanel({ run }: { run: OrchestrationRunState }) {
  const subagentCardStyle = useUiCustomizationStore((s) => s.subagentCardStyle);
  const props = orchestrationRunToSubagentProps(run);

  return <BeuiSubagentCard {...props} variant={subagentCardStyle} />;
}
