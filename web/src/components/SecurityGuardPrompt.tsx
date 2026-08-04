import React from 'react';
import type { SecurityPromptInfo } from '../types';
import { ShieldAlert, Check, X, Loader2, ShieldCheck } from 'lucide-react';

interface SecurityGuardPromptProps {
  prompt: SecurityPromptInfo;
  onChoice: (choice: 'approve' | 'deny') => void;
}

export const SecurityGuardPrompt: React.FC<SecurityGuardPromptProps> = ({ prompt, onChoice }) => {
  const argsPreview = (() => {
    if (!prompt.arguments) return '';
    if (typeof prompt.arguments === 'string') return prompt.arguments;
    try {
      return JSON.stringify(prompt.arguments, null, 2);
    } catch {
      return String(prompt.arguments);
    }
  })();

  return (
    <div className="my-3 overflow-hidden rounded-xl border border-amber-500/40 bg-amber-500/10 text-xs text-amber-200 shadow-sm backdrop-blur-sm">
      <div className="flex items-start gap-2.5 p-3.5">
        <div className="rounded-full bg-amber-500/20 p-1 text-amber-400">
          <ShieldAlert className="h-4 w-4" />
        </div>
        <div className="min-w-0 flex-1">
          <div className="font-semibold text-amber-300">SecurityGuard Interception</div>
          <div className="mt-1 text-[11px] text-amber-200/80">
            OpenZ is requesting to run a sensitive operation:
          </div>
          <div className="mt-2 rounded-lg bg-black/40 p-2 font-mono text-[11px] text-amber-100 border border-amber-500/20">
            <span className="font-bold text-amber-300">[{prompt.toolName}]</span>
            {prompt.description && (
              <span className="text-amber-200/90"> {prompt.description}</span>
            )}
          </div>
          {argsPreview && (
            <pre className="mt-1.5 max-h-40 overflow-x-auto rounded bg-black/30 p-2 font-mono text-[10px] text-amber-100/80 border border-amber-500/15">
              {argsPreview}
            </pre>
          )}

          {prompt.status === 'pending' ? (
            <div className="mt-3 flex items-center gap-2">
              <button
                onClick={() => onChoice('approve')}
                className="flex items-center gap-1 rounded-lg bg-amber-500 px-3 py-1.5 text-[11px] font-semibold text-black hover:bg-amber-400 transition focus:outline-none focus-visible:ring-2 focus-visible:ring-amber-400 focus-visible:ring-offset-1 focus-visible:ring-offset-black"
              >
                <ShieldCheck className="h-3 w-3" /> Approve Execution
              </button>
              <button
                onClick={() => onChoice('deny')}
                className="flex items-center gap-1 rounded-lg border border-amber-500/40 bg-transparent px-3 py-1.5 text-[11px] font-medium text-amber-300 hover:bg-amber-500/20 transition focus:outline-none focus-visible:ring-2 focus-visible:ring-amber-400/60"
              >
                <X className="h-3 w-3" /> Deny
              </button>
            </div>
          ) : (
            <div className="mt-2.5 flex items-center gap-1.5 font-medium text-[11px]">
              {prompt.status === 'approved' ? (
                <>
                  <Check className="h-3 w-3 text-emerald-400" />
                  <span className="text-emerald-400">Approved — execution authorized</span>
                </>
              ) : (
                <>
                  <X className="h-3 w-3 text-red-400" />
                  <span className="text-red-400">Denied — execution blocked</span>
                </>
              )}
            </div>
          )}

          {prompt.status === 'pending' && (
            <div className="mt-2 flex items-center gap-1 text-[10px] text-amber-300/60">
              <Loader2 className="h-3 w-3 animate-spin" /> Awaiting your decision — the turn is paused
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
