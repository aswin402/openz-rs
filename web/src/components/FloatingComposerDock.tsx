import React from 'react';
import { ChatTrackRow } from './ChatTrackRow';
import { ChatInput } from '../features/chat/ChatInput';
import { useUiCustomizationStore } from '../store/useUiCustomizationStore';
import { cn } from '../lib/utils';

export interface FloatingComposerDockProps {
  className?: string;
}

/**
 * Standalone floating WebUI Composer Dock.
 * Anchored identically to `WorkspaceRightDock` (`absolute bottom-3 md:bottom-4 z-20`)
 * with `pointer-events-none` and zero full-width background slab so chat messages
 * glide cleanly beneath the frosted glass input card.
 */
export const FloatingComposerDock: React.FC<FloatingComposerDockProps> = ({
  className,
}) => {
  const chatWidth = useUiCustomizationStore((s) => s.chatWidth);

  return (
    <div
      className={cn(
        'pointer-events-none absolute inset-x-0 bottom-3 z-20 bg-transparent md:bottom-4',
        className,
      )}
    >
      <ChatTrackRow
        chatWidth={chatWidth}
        className="bg-transparent"
        centerClassName="pointer-events-auto relative bg-transparent"
      >
        <ChatInput embeddedInFloatingDock />
      </ChatTrackRow>
    </div>
  );
};
