//! A mirror preset turned into a job the job runner can run (plan 7).

use std::path::Path;
use std::sync::Arc;

use secopy_core::mirror::{self, MirrorOptions, MirrorPlan};

use crate::session::{Ready, gone};
use crate::store::MirrorPreset;

/// A mirror worked out and ready to run.
#[derive(Debug, Clone)]
pub struct MirrorJob {
    pub plan: Arc<MirrorPlan>,
    pub name: String,
}

/// Plans `preset` now; a missing origin or destination says so, like New copy does.
pub fn prepare(preset: &MirrorPreset) -> Result<MirrorJob, String> {
    for p in [&preset.origin, &preset.destination] {
        let path = Path::new(p);
        if !path.is_dir() {
            return Err(gone(path));
        }
    }
    let options = MirrorOptions {
        deleted: (&preset.deleted).into(),
        deep_check: preset.deep_check,
    };
    let plan = mirror::plan(
        Path::new(&preset.origin),
        Path::new(&preset.destination),
        &options,
    )?;
    Ok(MirrorJob {
        plan: Arc::new(plan),
        name: preset.name.clone(),
    })
}

impl MirrorJob {
    /// The copy phase, as the job runner takes it.
    pub fn ready(&self) -> Ready {
        Ready {
            source: self.plan.source.clone(),
            plan: self.plan.copy.clone(),
            label: format!("Mirror · {}", self.name),
            copy_root: self.plan.copy.dest.clone(),
        }
    }
}
