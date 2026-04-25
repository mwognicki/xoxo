use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillDescriptor {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedSkill {
    pub skill_name: String,
    pub skill_description: String,
    pub path: PathBuf,
}

#[derive(Debug, Error)]
pub enum LearnSkillError {
    #[error("skill name must not be empty")]
    EmptySkillName,
    #[error("search path must be absolute: {0}")]
    RelativeSearchPath(String),
    #[error("skill {skill_name:?} was not found in: {searched_locations}")]
    NotFound {
        skill_name: String,
        searched_locations: String,
    },
    #[error("failed to read skill {skill_name:?} from {path}: {source}")]
    ReadFailed {
        skill_name: String,
        path: String,
        source: std::io::Error,
    },
}

#[derive(Debug, Error)]
pub enum ListSkillsError {
    #[error("search path must be absolute: {0}")]
    RelativeSearchPath(String),
}

/// Discover skills available from supported project and global directories.
pub fn discover_available_skills() -> Vec<SkillDescriptor> {
    let current_dir = std::env::current_dir().ok();
    let home_dir = std::env::var_os("HOME").map(PathBuf::from);
    discover_available_skills_from(current_dir.as_deref(), home_dir.as_deref())
}

/// Load the full markdown content of a skill by name.
///
/// Search order is:
/// 1. Caller-provided absolute directories, in order
/// 2. `./.xoxo/skills`
/// 3. `./.agents/skills`
/// 4. `~/.xoxo/skills`
///
/// The first match wins.
pub fn learn_skill(
    skill_name: &str,
    search_paths: &[PathBuf],
) -> Result<String, LearnSkillError> {
    let current_dir = std::env::current_dir().ok();
    let home_dir = std::env::var_os("HOME").map(PathBuf::from);
    learn_skill_from(skill_name, search_paths, current_dir.as_deref(), home_dir.as_deref())
}

/// List all discovered skills from caller-provided directories and default roots.
///
/// Search order is:
/// 1. Caller-provided absolute directories, in order
/// 2. `./.xoxo/skills`
/// 3. `./.agents/skills`
/// 4. `~/.xoxo/skills`
///
/// If the same skill name appears multiple times, the first match wins.
pub fn list_skills(search_paths: &[PathBuf]) -> Result<Vec<ListedSkill>, ListSkillsError> {
    let current_dir = std::env::current_dir().ok();
    let home_dir = std::env::var_os("HOME").map(PathBuf::from);
    list_skills_from(search_paths, current_dir.as_deref(), home_dir.as_deref())
}

fn discover_available_skills_from(
    current_dir: Option<&Path>,
    home_dir: Option<&Path>,
) -> Vec<SkillDescriptor> {
    let mut skills = Vec::new();
    let mut seen = HashSet::new();

    let search_roots = [
        current_dir.map(|dir| dir.join(".xoxo/skills")),
        current_dir.map(|dir| dir.join(".agents/skills")),
        home_dir.map(|dir| dir.join(".xoxo/skills")),
    ];

    for root in search_roots.into_iter().flatten() {
        collect_skills_from_root(&root, &mut seen, &mut skills);
    }

    skills.sort_by(|left, right| left.name.cmp(&right.name));
    skills
}

fn learn_skill_from(
    skill_name: &str,
    search_paths: &[PathBuf],
    current_dir: Option<&Path>,
    home_dir: Option<&Path>,
) -> Result<String, LearnSkillError> {
    let skill_name = skill_name.trim();
    if skill_name.is_empty() {
        return Err(LearnSkillError::EmptySkillName);
    }

    for path in search_paths {
        if !path.is_absolute() {
            return Err(LearnSkillError::RelativeSearchPath(
                path.display().to_string(),
            ));
        }
    }

    let mut roots = search_paths.to_vec();
    roots.extend(default_skill_roots(current_dir, home_dir));

    let searched_locations = roots
        .iter()
        .map(|root| root.join(skill_name).join("SKILL.md").display().to_string())
        .collect::<Vec<_>>();

    for skill_file in roots
        .into_iter()
        .map(|root| root.join(skill_name).join("SKILL.md"))
    {
        if !skill_file.is_file() {
            continue;
        }

        return fs::read_to_string(&skill_file).map_err(|source| LearnSkillError::ReadFailed {
            skill_name: skill_name.to_string(),
            path: skill_file.display().to_string(),
            source,
        });
    }

    Err(LearnSkillError::NotFound {
        skill_name: skill_name.to_string(),
        searched_locations: searched_locations.join(", "),
    })
}

fn default_skill_roots(current_dir: Option<&Path>, home_dir: Option<&Path>) -> Vec<PathBuf> {
    [
        current_dir.map(|dir| dir.join(".xoxo/skills")),
        current_dir.map(|dir| dir.join(".agents/skills")),
        home_dir.map(|dir| dir.join(".xoxo/skills")),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn list_skills_from(
    search_paths: &[PathBuf],
    current_dir: Option<&Path>,
    home_dir: Option<&Path>,
) -> Result<Vec<ListedSkill>, ListSkillsError> {
    validate_search_paths(search_paths).map_err(ListSkillsError::RelativeSearchPath)?;

    let mut skills = Vec::new();
    let mut seen = HashSet::new();
    let mut roots = search_paths.to_vec();
    roots.extend(default_skill_roots(current_dir, home_dir));

    for root in roots {
        collect_listed_skills_from_root(&root, &mut seen, &mut skills);
    }

    skills.sort_by(|left, right| left.skill_name.cmp(&right.skill_name));
    Ok(skills)
}

fn collect_skills_from_root(
    root: &Path,
    seen: &mut HashSet<String>,
    skills: &mut Vec<SkillDescriptor>,
) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };

    for entry in entries.flatten() {
        let skill_file = entry.path().join("SKILL.md");
        if !skill_file.is_file() {
            continue;
        }

        let Ok(contents) = fs::read_to_string(&skill_file) else {
            continue;
        };

        let Some(skill) = parse_skill_front_matter(&contents) else {
            continue;
        };

        if seen.insert(skill.name.clone()) {
            skills.push(skill);
        }
    }
}

fn collect_listed_skills_from_root(
    root: &Path,
    seen: &mut HashSet<String>,
    skills: &mut Vec<ListedSkill>,
) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };

    for entry in entries.flatten() {
        let skill_file = entry.path().join("SKILL.md");
        if !skill_file.is_file() {
            continue;
        }

        let Ok(contents) = fs::read_to_string(&skill_file) else {
            continue;
        };

        let Some(skill) = parse_skill_front_matter(&contents) else {
            continue;
        };

        if seen.insert(skill.name.clone()) {
            skills.push(ListedSkill {
                skill_name: skill.name,
                skill_description: skill.description,
                path: skill_file,
            });
        }
    }
}

fn parse_skill_front_matter(contents: &str) -> Option<SkillDescriptor> {
    let mut lines = contents.lines();
    if lines.next()? != "---" {
        return None;
    }

    let mut name = None;
    let mut description = None;

    for line in lines {
        if line == "---" {
            break;
        }

        let (key, value) = line.split_once(':')?;
        let value = value.trim().trim_matches('"').trim_matches('\'');
        match key.trim() {
            "name" if !value.is_empty() => name = Some(value.to_string()),
            "description" if !value.is_empty() => description = Some(value.to_string()),
            _ => {}
        }
    }

    Some(SkillDescriptor {
        name: name?,
        description: description?,
    })
}

fn validate_search_paths(search_paths: &[PathBuf]) -> Result<(), String> {
    for path in search_paths {
        if !path.is_absolute() {
            return Err(path.display().to_string());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        LearnSkillError, ListSkillsError, discover_available_skills_from, learn_skill_from,
        list_skills_from, parse_skill_front_matter,
    };
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn parses_skill_front_matter() {
        let skill = parse_skill_front_matter(
            "---\nname: rust-best-practices\ndescription: Shared Rust guidance.\n---\n# Body\n",
        )
        .expect("skill should parse");

        assert_eq!(skill.name, "rust-best-practices");
        assert_eq!(skill.description, "Shared Rust guidance.");
    }

    #[test]
    fn ignores_missing_front_matter() {
        assert!(parse_skill_front_matter("# Not a skill\n").is_none());
    }

    #[test]
    fn discovers_project_and_global_skills_with_project_precedence() {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let home = tempfile::tempdir().expect("home tempdir");

        write_skill(
            workspace.path(),
            ".xoxo/skills/rust-best-practices/SKILL.md",
            "rust-best-practices",
            "Project Rust guidance.",
        );
        write_skill(
            workspace.path(),
            ".agents/skills/deployment/SKILL.md",
            "deployment",
            "Project deployment help.",
        );
        write_skill(
            home.path(),
            ".xoxo/skills/rust-best-practices/SKILL.md",
            "rust-best-practices",
            "Global Rust guidance.",
        );
        write_skill(
            home.path(),
            ".xoxo/skills/terraform/SKILL.md",
            "terraform",
            "Global Terraform help.",
        );

        let skills =
            discover_available_skills_from(Some(workspace.path()), Some(home.path()));

        assert_eq!(
            skills.iter().map(|skill| skill.name.as_str()).collect::<Vec<_>>(),
            vec!["deployment", "rust-best-practices", "terraform"]
        );
        assert_eq!(
            skills
                .iter()
                .find(|skill| skill.name == "rust-best-practices")
                .expect("rust skill")
                .description,
            "Project Rust guidance."
        );
    }

    #[test]
    fn learns_skill_from_custom_paths_before_defaults() {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let custom = tempfile::tempdir().expect("custom tempdir");
        let home = tempfile::tempdir().expect("home tempdir");

        write_skill(
            custom.path(),
            "rust-best-practices/SKILL.md",
            "rust-best-practices",
            "Custom Rust guidance.",
        );
        write_skill(
            workspace.path(),
            ".xoxo/skills/rust-best-practices/SKILL.md",
            "rust-best-practices",
            "Project Rust guidance.",
        );

        let content = learn_skill_from(
            "rust-best-practices",
            &[custom.path().to_path_buf()],
            Some(workspace.path()),
            Some(home.path()),
        )
        .expect("skill should be found");

        assert!(content.contains("Custom Rust guidance."));
    }

    #[test]
    fn lists_skills_from_custom_and_default_paths_with_first_match_winning() {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let custom = tempfile::tempdir().expect("custom tempdir");
        let home = tempfile::tempdir().expect("home tempdir");

        write_skill(
            custom.path(),
            "rust-best-practices/SKILL.md",
            "rust-best-practices",
            "Custom Rust guidance.",
        );
        write_skill(
            workspace.path(),
            ".xoxo/skills/rust-best-practices/SKILL.md",
            "rust-best-practices",
            "Project Rust guidance.",
        );
        write_skill(
            workspace.path(),
            ".agents/skills/deployment/SKILL.md",
            "deployment",
            "Project deployment help.",
        );

        let skills = list_skills_from(
            &[custom.path().to_path_buf()],
            Some(workspace.path()),
            Some(home.path()),
        )
        .expect("list skills should succeed");

        assert_eq!(
            skills
                .iter()
                .map(|skill| skill.skill_name.as_str())
                .collect::<Vec<_>>(),
            vec!["deployment", "rust-best-practices"]
        );
        let rust_skill = skills
            .iter()
            .find(|skill| skill.skill_name == "rust-best-practices")
            .expect("rust skill");
        assert_eq!(rust_skill.skill_description, "Custom Rust guidance.");
        assert_eq!(
            rust_skill.path,
            custom.path().join("rust-best-practices").join("SKILL.md")
        );
    }

    #[test]
    fn list_skills_rejects_relative_search_paths() {
        let error = list_skills_from(&[PathBuf::from("./relative")], None, None)
            .expect_err("relative path should fail");

        assert_eq!(
            error.to_string(),
            ListSkillsError::RelativeSearchPath("./relative".to_string()).to_string()
        );
    }

    #[test]
    fn learns_skill_from_defaults_when_custom_paths_miss() {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let custom = tempfile::tempdir().expect("custom tempdir");
        let home = tempfile::tempdir().expect("home tempdir");

        write_skill(
            workspace.path(),
            ".agents/skills/deployment/SKILL.md",
            "deployment",
            "Project deployment help.",
        );

        let content = learn_skill_from(
            "deployment",
            &[custom.path().to_path_buf()],
            Some(workspace.path()),
            Some(home.path()),
        )
        .expect("skill should be found");

        assert!(content.contains("Project deployment help."));
    }

    #[test]
    fn rejects_relative_search_paths() {
        let error = learn_skill_from(
            "deployment",
            &[PathBuf::from("./relative")],
            None,
            None,
        )
        .expect_err("relative path should fail");

        assert_eq!(
            error.to_string(),
            LearnSkillError::RelativeSearchPath("./relative".to_string()).to_string()
        );
    }

    #[test]
    fn returns_clear_not_found_error() {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let home = tempfile::tempdir().expect("home tempdir");

        let error = learn_skill_from("missing", &[], Some(workspace.path()), Some(home.path()))
            .expect_err("missing skill should fail");

        match error {
            LearnSkillError::NotFound {
                skill_name,
                searched_locations,
            } => {
                assert_eq!(skill_name, "missing");
                assert!(searched_locations.contains(".xoxo/skills/missing/SKILL.md"));
                assert!(searched_locations.contains(".agents/skills/missing/SKILL.md"));
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    fn write_skill(base: &std::path::Path, relative: &str, name: &str, description: &str) {
        let path = base.join(relative);
        fs::create_dir_all(path.parent().expect("skill dir")).expect("create skill dir");
        fs::write(
            path,
            format!(
                "---\nname: {name}\ndescription: {description}\n---\n# {name}\n"
            ),
        )
        .expect("write skill");
    }
}
