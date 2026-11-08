//! Host-owned worker and live-memory policy for streaming encryption copies.
use super::Error;
use tablecraft_ace::InspectError;

const MAX_WORKERS: u16 = 256;
const MIB_BYTES: u64 = 1024 * 1024;
const HEADROOM_DIVISOR: u64 = 32;
const MIN_HEADROOM_BYTES: u64 = 16 * MIB_BYTES;
const MAX_HEADROOM_BYTES: u64 = 512 * MIB_BYTES;
#[cfg(target_os = "linux")]
const UNKNOWN_CGROUP_BATCH_BYTES: u64 = 4 * MIB_BYTES;

#[derive(clap::Args)]
pub(crate) struct Options {
    /// Worker threads; defaults to available parallelism (at most 256). Use 1 for serial work.
    #[arg(long, value_parser = clap::value_parser!(u16).range(1..=i64::from(MAX_WORKERS)))]
    jobs: Option<u16>,
    /// Maximum live encryption batch buffers in MiB; RAM headroom is still reserved.
    #[arg(long, value_name = "MIB", value_parser = clap::value_parser!(u64).range(1..))]
    memory_budget_mib: Option<u64>,
}

impl Options {
    pub(super) fn workers(&self) -> usize {
        self.jobs.map_or_else(
            || {
                std::thread::available_parallelism()
                    .map_or(1, std::num::NonZeroUsize::get)
                    .min(usize::from(MAX_WORKERS))
            },
            usize::from,
        )
    }

    fn memory_limit(&self) -> Result<Option<u64>, Error> {
        self.memory_budget_mib
            .map(|mib| {
                mib.checked_mul(MIB_BYTES)
                    .ok_or(Error::Read(InspectError::Allocation))
            })
            .transpose()
    }
}

#[cfg(not(target_family = "wasm"))]
pub(super) struct Memory {
    system: sysinfo::System,
    limit: Option<u64>,
    #[cfg(target_os = "linux")]
    reported_unknown_controller: bool,
}

#[cfg(not(target_family = "wasm"))]
impl Memory {
    pub(super) fn new(options: &Options) -> Result<Self, Error> {
        let system = sysinfo::System::new_with_specifics(
            sysinfo::RefreshKind::nothing()
                .with_memory(sysinfo::MemoryRefreshKind::nothing().with_ram()),
        )
        .map_err(|_| Error::Read(InspectError::Allocation))?;
        #[cfg(target_os = "linux")]
        let mut system = system;
        #[cfg(target_os = "linux")]
        {
            let pid = sysinfo::Pid::from_u32(std::process::id());
            system.refresh_processes_specifics(
                sysinfo::ProcessesToUpdate::Some(&[pid]),
                false,
                sysinfo::ProcessRefreshKind::nothing().without_tasks(),
            );
        }
        Ok(Self {
            system,
            limit: options.memory_limit()?,
            #[cfg(target_os = "linux")]
            reported_unknown_controller: false,
        })
    }

    pub(super) fn allowance(&mut self, retained: usize) -> Result<usize, InspectError> {
        self.system
            .refresh_memory_specifics(sysinfo::MemoryRefreshKind::nothing().with_ram());
        let total = self.system.total_memory();
        let available = self.system.available_memory();
        let limit = self.limit;
        #[cfg(target_os = "linux")]
        let (total, available, limit) = {
            let pid = sysinfo::Pid::from_u32(std::process::id());
            let limits = self
                .system
                .process(pid)
                .and_then(sysinfo::Process::cgroup_limits);
            match limits {
                Some(limits) => (
                    total.min(limits.total_memory),
                    available.min(limits.free_memory),
                    limit,
                ),
                // None also represents discovery/read failure, so automatic
                // sizing stays conservative. An explicit cap is user policy;
                // it still cannot exceed the observed host RAM/headroom.
                None => {
                    if !self.reported_unknown_controller {
                        tracing::debug!("encryption memory controller limit unavailable");
                        self.reported_unknown_controller = true;
                    }
                    (
                        total,
                        available,
                        Some(limit.unwrap_or(UNKNOWN_CGROUP_BATCH_BYTES)),
                    )
                }
            }
        };
        allowance(total, available, retained, limit)
    }
}

fn allowance(
    total: u64,
    available: u64,
    retained: usize,
    explicit: Option<u64>,
) -> Result<usize, InspectError> {
    // Available RAM excludes buffers already held by this operation. Add only
    // that retained working set back before applying the next total-buffer cap.
    let retained = u64::try_from(retained).map_err(|_| InspectError::Allocation)?;
    let available = available.saturating_add(retained).min(total);
    let headroom = (total / HEADROOM_DIVISOR)
        .clamp(MIN_HEADROOM_BYTES, MAX_HEADROOM_BYTES)
        .min(total / 4);
    let remaining = available.saturating_sub(headroom);
    let allowed = explicit.map_or(remaining, |limit| limit.min(remaining));
    if allowed == 0 {
        return Err(InspectError::Allocation);
    }
    usize::try_from(allowed.min(isize::MAX as u64)).map_err(|_| InspectError::Allocation)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_buffers_are_not_double_subtracted_and_explicit_cap_stays_total() {
        let total = 8 * 1024 * MIB_BYTES;
        let free_before = 2 * 1024 * MIB_BYTES;
        let retained = 64 * MIB_BYTES;
        let first = allowance(total, free_before, 0, None).unwrap();
        let reuse = allowance(total, free_before - retained, retained as usize, None).unwrap();
        assert_eq!(reuse, first);
        assert_eq!(
            allowance(total, free_before, retained as usize, Some(8 * MIB_BYTES)).unwrap(),
            (8 * MIB_BYTES) as usize
        );
    }

    #[test]
    fn ram_pressure_wins_over_requested_budget_and_tiny_hosts_keep_headroom() {
        let total = 8 * 1024 * MIB_BYTES;
        assert!(allowance(total, 128 * MIB_BYTES, 0, Some(1024 * MIB_BYTES)).is_err());
        assert_eq!(
            allowance(32 * MIB_BYTES, 16 * MIB_BYTES, 0, None).unwrap(),
            (8 * MIB_BYTES) as usize
        );
        assert!(allowance(0, 0, 0, None).is_err());
    }
}
