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
            match crate::agent::skills::inspect_skill_package(&name, profile.as_deref())? {
                None => anyhow::bail!("Skill '{}' not found.", name),
                Some((skill, refs, scripts)) => {
                    println!("=== Skill: {} ===", skill.name);
                    println!("Description: {}", skill.description());
                    let trigs = skill.triggers();
                    let _ = (!trigs.is_empty())
                        .then(|| println!("Triggers:    {}", trigs.join(", ")));
                    let _ = (!refs.is_empty())
                        .then(|| println!("References:  {}", refs.join(", ")));
                    let _ = (!scripts.is_empty())
                        .then(|| println!("Scripts:     {}", scripts.join(", ")));
                    println!("---\n{}", skill.content);
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
    }
    Ok(())
}
