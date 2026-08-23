use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStage {
    Validate,
    Decode,
    ResolveSettings,
    Develop,
    SceneRender,
    Encode,
    Complete,
}

#[derive(Clone, Debug, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

pub trait ProgressSink {
    fn stage_completed(&mut self, stage: JobStage);
}

#[derive(Default)]
pub struct NoProgress;

impl ProgressSink for NoProgress {
    fn stage_completed(&mut self, _stage: JobStage) {}
}
