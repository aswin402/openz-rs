import React, { useState } from 'react';
import { useOpenZStore } from '../store/useOpenZStore';
import { BookOpen, ChevronRight, FileText, Search } from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

export const SkillsView: React.FC = () => {
  const skills = useOpenZStore((s) => s.skills);
  const [selectedSkill, setSelectedSkill] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');

  const filteredSkills = skills.filter((skill) =>
    skill.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
    skill.content.toLowerCase().includes(searchQuery.toLowerCase())
  );

  const activeSkill = skills.find((s) => s.name === selectedSkill);

  return (
    <div className="mx-auto max-w-5xl px-4 py-6 h-full flex flex-col overflow-hidden">
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-4 pb-4 border-b border-border/50">
        <div>
          <h1 className="flex items-center gap-2 text-2xl font-extrabold tracking-tight text-foreground">
            <BookOpen className="h-5 w-5 text-amber-500" /> AI Agent Skills
          </h1>
          <p className="mt-1 text-sm text-muted-foreground">
            Dynamic skills and prompt guidelines loaded from <code>~/.openz/skills/*.md</code>
          </p>
        </div>

        <div className="relative w-full md:w-64">
          <Search className="absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" />
          <input
            type="text"
            placeholder="Search skills..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full rounded-lg border border-border/60 bg-card/60 pl-9 pr-4 py-1.5 text-xs text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-amber-500/50"
          />
        </div>
      </div>

      <div className="mt-6 flex-1 flex gap-6 min-h-0">
        {/* Left Side: Skills List */}
        <div className={`flex-1 flex flex-col gap-2 overflow-y-auto pr-1 ${selectedSkill ? 'hidden md:flex max-w-xs' : 'w-full'}`}>
          {filteredSkills.length === 0 ? (
            <div className="py-12 text-center text-xs text-muted-foreground border border-dashed border-border/60 rounded-2xl bg-muted/10">
              No skills found. Start using the TUI or WebUI to create custom skills.
            </div>
          ) : (
            filteredSkills.map((skill) => {
              const isSelected = skill.name === selectedSkill;
              return (
                <button
                  key={skill.name}
                  onClick={() => setSelectedSkill(isSelected ? null : skill.name)}
                  className={`flex items-center justify-between w-full p-3.5 rounded-xl border text-left transition select-none ${
                    isSelected
                      ? 'border-amber-500/40 bg-amber-500/10 text-amber-200'
                      : 'border-border/60 bg-card/40 text-foreground hover:border-border hover:bg-muted/20'
                  }`}
                >
                  <div className="flex items-center gap-3 min-w-0">
                    <div className={`flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border ${
                      isSelected ? 'border-amber-500/20 bg-amber-500/20' : 'border-border/80 bg-muted/40'
                    }`}>
                      <FileText className={`h-4 w-4 ${isSelected ? 'text-amber-400' : 'text-muted-foreground'}`} />
                    </div>
                    <div className="truncate">
                      <div className="font-semibold text-xs leading-normal truncate">{skill.name}</div>
                      <div className="text-[10px] text-muted-foreground mt-0.5 truncate max-w-[200px]">
                        {skill.content.slice(0, 60).replace(/[#*`_-]/g, '')}...
                      </div>
                    </div>
                  </div>
                  <ChevronRight className={`h-4 w-4 shrink-0 transition-transform ${isSelected ? 'text-amber-500 rotate-90 md:rotate-0' : 'text-muted-foreground'}`} />
                </button>
              );
            })
          )}
        </div>

        {/* Right Side: Skill Content View */}
        {selectedSkill && activeSkill ? (
          <div className="flex-1 flex flex-col border border-border/80 bg-card/40 rounded-2xl min-h-0 overflow-hidden shadow-sm">
            <div className="flex items-center justify-between px-4 py-3 bg-muted/30 border-b border-border/60">
              <div className="flex items-center gap-2">
                <FileText className="h-4 w-4 text-amber-500" />
                <span className="font-bold text-xs text-foreground">{activeSkill.name}</span>
              </div>
              <button
                onClick={() => setSelectedSkill(null)}
                className="text-[11px] font-semibold text-amber-500 hover:text-amber-400 transition"
              >
                Close View
              </button>
            </div>
            <div className="flex-1 overflow-y-auto p-5 prose dark:prose-invert prose-xs max-w-none break-words select-text">
              <ReactMarkdown remarkPlugins={[remarkGfm]}>
                {activeSkill.content}
              </ReactMarkdown>
            </div>
          </div>
        ) : (
          <div className="hidden md:flex flex-1 flex-col items-center justify-center border border-dashed border-border/60 bg-muted/5 rounded-2xl text-center p-6">
            <BookOpen className="h-10 w-10 text-muted-foreground/40 mb-3" />
            <h2 className="text-sm font-bold text-foreground">Select a Skill</h2>
            <p className="text-xs text-muted-foreground mt-1 max-w-xs">
              Click any skill on the left to read its system-injected markdown content.
            </p>
          </div>
        )}
      </div>
    </div>
  );
};
