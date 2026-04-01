use anyhow::Result;

use super::queue::BuildJob;
use super::ram_guard::RamGuard;
use crate::supervisor::ProcessSupervisor;

/// Old Docker-based 5-phase builder. Superseded by router::run_build_and_send.
#[derive(Clone)]
pub struct ParallelBuilder {
    _ram_guard: std::sync::Arc<RamGuard>,
    _supervisor: ProcessSupervisor,
}

impl ParallelBuilder {
    pub fn new(ram_guard: RamGuard, supervisor: ProcessSupervisor) -> Self {
        Self {
            _ram_guard: std::sync::Arc::new(ram_guard),
            _supervisor: supervisor,
        }
    }

    pub async fn run_planning(&self, _job: &mut BuildJob) -> Result<()> {
        anyhow::bail!("Old builder deprecated — use /approve in #ideas topic")
    }

    pub async fn run_coding(&self, _job: &BuildJob) -> Result<()> {
        anyhow::bail!("Old builder deprecated — use /approve in #ideas topic")
    }

    pub async fn run_testing(&self, _job: &BuildJob) -> Result<()> {
        anyhow::bail!("Old builder deprecated — use /approve in #ideas topic")
    }

    pub async fn run_docker_build(&self, _job: &BuildJob) -> Result<()> {
        anyhow::bail!("Old builder deprecated — use /approve in #ideas topic")
    }

    pub async fn run_deploy(&self, _job: &BuildJob) -> Result<()> {
        anyhow::bail!("Old builder deprecated — use /approve in #ideas topic")
    }
}
