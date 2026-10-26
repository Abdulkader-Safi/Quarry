//! Agent permissions (Stage 05 "Permissions"): agents may read any path they are
//! given but write only inside the open project's folder or the folder the
//! headless runner was started in. Paths are canonicalised, symlinks resolved,
//! before the check.

use quarry_core::{CommandError, ErrorCode};
use serde_json::Value;
use std::path::{Component, Path, PathBuf};

/// Commands that write files, and the argument naming where.
const WRITES: &[(&str, &str)] = &[
    ("project.new", "path"),
    ("project.save_as", "path"),
    ("export.run", "path"),
];

/// Commands that pick which file later saves go to: the agent may only aim
/// them inside its folders, or it could open a project elsewhere and save there.
const RETARGETS: &[(&str, &str)] = &[("project.open", "path")];

/// Human-only: recovery writes to whatever file the autosave names.
const HUMAN_ONLY: &[&str] = &["project.recover"];

/// Refuse an agent request that would write outside `roots`. `saving_to` is
/// where `project.save` would write (the open project's file).
pub fn check_agent(
    command: &str,
    args: &Value,
    roots: &[PathBuf],
    saving_to: Option<&Path>,
) -> Result<(), CommandError> {
    if HUMAN_ONLY.contains(&command) {
        return Err(CommandError::new(
            ErrorCode::Forbidden,
            format!("{command} is for the person at the keyboard; agents cannot run it."),
        ));
    }
    if command == "project.save"
        && let Some(target) = saving_to
        && !inside(target, roots)
    {
        return Err(refuse(target, roots));
    }
    for (name, field) in WRITES.iter().chain(RETARGETS) {
        if *name != command {
            continue;
        }
        let Some(target) = args.get(*field).and_then(Value::as_str) else {
            continue;
        };
        let target = Path::new(target);
        if !inside(target, roots) {
            return Err(refuse(target, roots));
        }
    }
    Ok(())
}

fn inside(target: &Path, roots: &[PathBuf]) -> bool {
    resolve(target).is_some_and(|t| {
        roots
            .iter()
            .filter_map(|r| r.canonicalize().ok())
            .any(|root| t.starts_with(root))
    })
}

fn refuse(target: &Path, roots: &[PathBuf]) -> CommandError {
    let folders: Vec<String> = roots.iter().map(|r| r.display().to_string()).collect();
    CommandError::new(
        ErrorCode::Forbidden,
        format!(
            "Agents can only write inside the project folder ({}). {} is outside it; pick a path in that folder.",
            if folders.is_empty() {
                "none is open yet".into()
            } else {
                folders.join(", ")
            },
            target.display()
        ),
    )
}

/// The real location `path` would be written to: the nearest existing ancestor,
/// canonicalised, plus the rest. None for relative paths or `..` past that ancestor.
fn resolve(path: &Path) -> Option<PathBuf> {
    if !path.is_absolute() {
        return None;
    }
    let mut existing = path;
    let mut rest = Vec::new();
    while !existing.exists() {
        rest.push(existing.file_name()?);
        existing = existing.parent()?;
    }
    let mut real = existing.canonicalize().ok()?;
    for part in rest.into_iter().rev() {
        if Path::new(part)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        {
            return None;
        }
        real.push(part);
    }
    Some(real)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn writes_outside_the_project_folder_are_refused() {
        let root = std::env::temp_dir().join(format!("quarry-perm-{}", std::process::id()));
        let project = root.join("project");
        std::fs::create_dir_all(&project).unwrap();
        let roots = [project.clone()];

        let ok = json!({ "path": project.join("sub/level.quarry") });
        assert!(check_agent("project.save_as", &ok, &roots, None).is_ok());

        for path in [
            root.join("other.quarry"),
            project.join("../escape.quarry"),
            PathBuf::from("relative.quarry"),
        ] {
            let err =
                check_agent("project.save_as", &json!({ "path": path }), &roots, None).unwrap_err();
            assert_eq!(err.code, ErrorCode::Forbidden, "{}", path.display());
        }
        // Images may be read from anywhere, but a project opened elsewhere
        // would make its folder the save target, so that is refused.
        assert!(check_agent("art.import", &json!({ "path": "/tmp/x.png" }), &roots, None).is_ok());
        let err = check_agent(
            "project.open",
            &json!({ "path": "/tmp/x.quarry" }),
            &roots,
            None,
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::Forbidden);
        let outside = root.join("elsewhere.quarry");
        assert!(check_agent("project.save", &json!({}), &roots, Some(&outside)).is_err());
        assert!(
            check_agent(
                "project.save",
                &json!({}),
                &roots,
                Some(&project.join("a.quarry"))
            )
            .is_ok()
        );
        assert!(
            check_agent(
                "project.recover",
                &json!({ "autosave": "/x" }),
                &roots,
                None
            )
            .is_err()
        );

        #[cfg(unix)]
        {
            let link = project.join("link");
            let _ = std::fs::remove_file(&link);
            std::os::unix::fs::symlink(&root, &link).unwrap();
            let via_link = json!({ "path": link.join("x.quarry") });
            assert!(
                check_agent("project.save_as", &via_link, &roots, None).is_err(),
                "symlinks are resolved"
            );
        }
    }
}
