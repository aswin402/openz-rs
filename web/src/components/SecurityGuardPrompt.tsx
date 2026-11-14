import React, { useMemo } from 'react';
import { ShieldAlert, Terminal } from 'lucide-react';
import type { SecurityPromptInfo } from '../types';
import {
  ApprovalCard,
  mapSecurityPromptToApprovalData,
  type ApprovalState,
  type ApprovalStyleChoice,
} from './assistant-ui/elements';
import { useUiCustomizationStore } from '../store/useUiCustomizationStore';
import { cn } from '../lib/utils';

export interface SecurityGuardPromptProps {
  prompt: SecurityPromptInfo;
  onChoice: (choice: 'approve' | 'deny') => void;
  stateOverride?: ApprovalState;
  styleOverride?: ApprovalStyleChoice;
  className?: string;
}

export const SecurityGuardPrompt: React.FC<SecurityGuardPromptProps> = ({
  prompt,
  onChoice,
  stateOverride,
  styleOverride,
  className,
}) => {
  const storeApprovalStyle = useUiCustomizationStore((s) => s.approvalStyle);
  const effectiveStyle = styleOverride ?? storeApprovalStyle;

  const mapped = useMemo(
    () =>
      mapSecurityPromptToApprovalData(prompt, {
        stateOverride,
        forceDestructive: effectiveStyle === 'assistant-destructive',
      }),
    [prompt, stateOverride, effectiveStyle],
  );

  return (
    <ApprovalCard
      state={mapped.state}
      title={mapped.title}
      subtitle={mapped.subtitle}
      description={mapped.description}
      command={mapped.command}
      details={mapped.details}
      variant={mapped.variant}
      icon={
        mapped.variant === 'destructive' ? (
          <ShieldAlert className="size-4" />
        ) : (
          <Terminal className="size-4" />
        )
      }
      onAllowOnce={() => onChoice('approve')}
      onDeny={() => onChoice('deny')}
      className={cn('max-w-xl', className)}
    />
  );
};
