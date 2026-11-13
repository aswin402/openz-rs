import React from 'react';
import type { SecurityPromptInfo } from '../types';
import { ApprovalCard, ToolApproval } from './beui';
import { useUiCustomizationStore } from '../store/useUiCustomizationStore';

interface SecurityGuardPromptProps {
  prompt: SecurityPromptInfo;
  onChoice: (choice: 'approve' | 'deny') => void;
}

export const SecurityGuardPrompt: React.FC<SecurityGuardPromptProps> = ({
  prompt,
  onChoice,
}) => {
  const approvalStyle = useUiCustomizationStore((s) => s.approvalStyle);

  if (approvalStyle === 'beui-approval-card') {
    return <ApprovalCard prompt={prompt} onChoice={onChoice} />;
  }

  return <ToolApproval prompt={prompt} onChoice={onChoice} />;
};
