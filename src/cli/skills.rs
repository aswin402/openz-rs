use super::args::SkillsAction;
use anyhow::Result;

pub async fn handle_skills(action: Option<SkillsAction>) -> Result<()> {
    let resolved = action.unwrap_or(SkillsAction::List { profile: None });
    match resolved {
        SkillsAction::List { profile } => {
            let views = crate::agent::skills::load_skill_views()?;
            let mut filtered: Vec<_> = views
                .into_iter()
                .filter(|v| match profile.as_deref() {
                    Some(prof) => v.profile.as_deref() == Some(prof),
                    None => true,
                })
                .collect();
            filtered.sort_by(|a, b| a.scope.cmp(&b.scope).then_with(|| a.name.cmp(&b.name)));
            match filtered.is_empty() {
                true => println!("No procedural skills found."),
                false => {
                    println!("OpenZ Procedural Skills ({} total):", filtered.len());
                    for s in filtered {
                        println!(
                            "  • {} [{} | uses={} ok={} revs={}] — {}",
                            s.name,
                            s.scope,
                            s.use_count,
                            s.success_count,
                            s.revision_count,
                            s.description
                        );
                    }
                }
            }
        }
        SkillsAction::Inspect { name, profile } => {
            match crate::agent::skills::inspect_skill_bundle(&name, profile.as_deref())? {
                None => anyhow::bail!("Skill '{}' not found.", name),
                Some(bundle) => {
                    println!("=== Skill: {} ===", bundle.skill.name);
                    println!("Description: {}", bundle.skill.description());
                    let trigs = bundle.skill.triggers();
                    let _ = (!trigs.is_empty())
                        .then(|| println!("Triggers:    {}", trigs.join(", ")));
                    let _ = (!bundle.references.is_empty())
                        .then(|| println!("References:  {}", bundle.references.join(", ")));
                    let _ = (!bundle.scripts.is_empty())
                        .then(|| println!("Scripts:     {}", bundle.scripts.join(", ")));
                    let _ = (!bundle.templates.is_empty())
                        .then(|| println!("Templates:   {}", bundle.templates.join(", ")));
                    let _ = (!bundle.assets.is_empty())
                        .then(|| println!("Assets:      {}", bundle.assets.join(", ")));
                    println!("---\n{}", bundle.skill.content);
                }
            }
        }
        SkillsAction::Delete { name, profile } => {
            crate::agent::skills::delete_skill_with_profile(&name, profile.as_deref())?;
            println!("Deleted skill '{}'.", name);
        }
        SkillsAction::History { name, profile } => {
            let revs = crate::agent::skills::list_skill_revisions(&name, profile.as_deref())?;
            match revs.is_empty() {
                true => println!("No revision snapshots recorded for '{}'.", name),
                false => {
                    println!("Revision history for '{}' ({} snapshots):", name, revs.len());
                    for r in revs {
                        println!(
                            "  • rev #{} ({}) — {} chars",
                            r.revision_id,
                            r.created_at,
                            r.content.chars().count()
                        );
                    }
                }
            }
        }
        SkillsAction::Rollback {
            name,
            revision_id,
            profile,
        } => {
            let restored =
                crate::agent::skills::rollback_skill(&name, profile.as_deref(), revision_id)?;
            println!("Rolled back skill '{}' cleanly.", restored.name);
        }
        SkillsAction::Archive => {
            let archived = crate::agent::skills::list_archived_skills()?;
            match archived.is_empty() {
                true => println!("No archived skills found in ~/.openz/skills/archive/."),
                false => {
                    println!("Archived OpenZ Skills ({} total):", archived.len());
                    for name in archived {
                        println!("  • {}", name);
                    }
                }
            }
        }
        SkillsAction::Restore { name } => {
            let restored = crate::agent::skills::restore_archived_skill(&name)?;
            println!(
                "Restored archived skill '{}' to ~/.openz/skills/{}/SKILL.md.",
                restored.name, restored.name
            );
        }
    }
    Ok(())
}
