//! project.json migrations, run in order on open. The file is saved back in the
//! newest format.

use serde_json::Value;

/// Bring `project` from format `from` up to `quarry_core::model::FORMAT_VERSION`.
/// Format 1 is the first, so there is nothing to do yet. Add steps as
/// `if from < 2 { … }` when the format changes.
pub fn run(project: &mut Value, from: u32) -> Result<(), String> {
    if from == 0 {
        return Err("it has format_version 0, which no Quarry wrote".into());
    }
    project["format_version"] = quarry_core::model::FORMAT_VERSION.into();
    Ok(())
}
