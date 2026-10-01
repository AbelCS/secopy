//! A mirror preset turned into a job the job runner can run (plan 7).

use std::path::Path;
use std::sync::Arc;

use secopy_core::control::JobControl;
use secopy_core::mirror::{self, MirrorOptions, MirrorPlan};

use crate::session::{Ready, gone};
use crate::store::MirrorPreset;

/// A mirror worked out and ready to run.
#[derive(Debug, Clone)]
pub struct MirrorJob {
    pub plan: Arc<MirrorPlan>,
    pub name: String,
    /// The preset's days: archived files older than this go at the start of a run, in Delete
    /// mode too (#101).
    pub archive_days: u32,
}

/// Plans `preset` now; a missing origin or destination says so, like New copy does. The deep
/// check reports to `on_compared` (files compared, of how many) and stops when `control` is
/// cancelled.
pub fn prepare(
    preset: &MirrorPreset,
    control: &JobControl,
    on_compared: &(dyn Fn(u64, u64) + Sync),
) -> Result<MirrorJob, crate::message::Message> {
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
    let plan = mirror::plan_watched(
        Path::new(&preset.origin),
        Path::new(&preset.destination),
        &options,
        control,
        on_compared,
    )
    .map_err(|e| crate::say::plan_error(&e))?;
    Ok(MirrorJob {
        plan: Arc::new(plan),
        name: preset.name.clone(),
        archive_days: preset.deleted.days,
    })
}

impl MirrorJob {
    /// The copy phase, as the job runner takes it.
    pub fn ready(&self) -> Ready {
        Ready {
            source: self.plan.source.clone(),
            plan: self.plan.copy.clone(),
            label: format!("Mirror · {}", self.name),
            shown: crate::message::Message::raw(crate::dto::show(&self.plan.origin)),
            mirror: Some(self.name.clone()),
            copy_root: self.plan.copy.dest.clone(),
            // Never for a mirror: it removes files a history would list (#154).
            mhl: None,
        }
    }
}
