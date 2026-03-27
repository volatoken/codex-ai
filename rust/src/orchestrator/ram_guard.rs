use sysinfo::System;
use tracing::warn;

/// Monitors system RAM and decides if new builds/processes can start.
pub struct RamGuard {
    total_ram_mb: u64,
    /// Minimum free MB to keep available (reserve for OS + bot)
    reserve_mb: u64,
}

impl RamGuard {
    pub fn new(total_ram_mb: u64) -> Self {
        // Reserve 20% of RAM for system
        let reserve_mb = total_ram_mb / 5;
        Self {
            total_ram_mb,
            reserve_mb,
        }
    }

    /// Returns currently available RAM in MB.
    pub fn available_mb(&self) -> u64 {
        let mut sys = System::new();
        sys.refresh_memory();
        sys.available_memory() / 1024 / 1024
    }

    /// Check if we can allocate `needed_mb` of RAM.
    pub fn can_allocate(&self, needed_mb: u64) -> bool {
        let available = self.available_mb();
        let ok = available > self.reserve_mb + needed_mb;
        if !ok {
            warn!(
                "RAM guard: cannot allocate {needed_mb}MB — available {available}MB, reserve {reserve}MB",
                reserve = self.reserve_mb
            );
        }
        ok
    }

    /// Block until enough RAM is available (polls every 5s).
    pub async fn wait_for(&self, needed_mb: u64) {
        while !self.can_allocate(needed_mb) {
            tracing::info!("Waiting for RAM... need {needed_mb}MB, have {}MB", self.available_mb());
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    }

    pub fn total_mb(&self) -> u64 {
        self.total_ram_mb
    }
}
