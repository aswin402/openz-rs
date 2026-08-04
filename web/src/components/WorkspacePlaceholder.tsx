import React from 'react';
import { useOpenZStore } from '../store/useOpenZStore';
import { Bot, BookOpen, MessageSquare } from 'lucide-react';

interface WorkspacePlaceholderProps {
  kind: 'agents' | 'skills';
}

const CONTENT: Record<WorkspacePlaceholderProps['kind'], { title: string; desc: string; icon: React.ComponentType<{ className?: string }> }> = {
  agents: {
    title: 'Agents',
    desc: 'Subagent profiles and tool orchestration live in the backend. A visual agent builder with model/provider, instructions, skills, MCP tools and security mode will surface here from real profile data.',
    icon: Bot,
  },
  skills: {
    title: 'Skills',
    desc: 'Skills are stored as markdown under ~/.openz/skills and injected into the system prompt. Browse, edit, create and test skills here once the skill CRUD contract is wired from the gateway.',
    icon: BookOpen,
  },
};

export const WorkspacePlaceholder: React.FC<WorkspacePlaceholderProps> = ({ kind }) => {
  const setActiveView = useOpenZStore((s) => s.setActiveView);
  const content = CONTENT[kind];
  const Icon = content.icon;

  return (
    <div className="flex h-full flex-col items-center justify-center px-6 text-center">
      <div className="flex h-16 w-16 items-center justify-center rounded-2xl border border-border/60 bg-card/60 shadow-sm">
        <Icon className="h-7 w-7 text-amber-500" />
      </div>
      <h1 className="mt-5 text-xl font-extrabold tracking-tight text-foreground">{content.title}</h1>
      <p className="mt-2 max-w-sm text-sm leading-relaxed text-muted-foreground">{content.desc}</p>
      <button
        onClick={() => setActiveView('chats')}
        className="mt-6 flex items-center gap-2 rounded-xl border border-amber-500/30 bg-amber-500/10 px-4 py-2 text-xs font-semibold text-amber-400 transition hover:bg-amber-500/20"
      >
        <MessageSquare className="h-4 w-4" /> Back to chats
      </button>
    </div>
  );
};