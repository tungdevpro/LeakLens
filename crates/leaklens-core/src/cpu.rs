//! CPU Usage from `cat /proc/<pid>/stat /proc/stat`, read in one call so both
//! counters come from the same moment.

/// CPU time counters from one reading, in clock ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CpuReading {
    /// Time the Target Process has spent running (utime + stime).
    process: u64,
    /// Time all cores together have spent in any state.
    total: u64,
    cores: u32,
}

impl CpuReading {
    pub(crate) fn parse(output: &str) -> Option<Self> {
        let mut lines = output.lines();
        let process = process_ticks(lines.next()?)?;
        let mut total = None;
        let mut cores = 0;
        for line in lines {
            let mut fields = line.split_whitespace();
            match fields.next() {
                // user nice system idle iowait irq softirq steal; guest time is already in user.
                Some("cpu") => {
                    total = Some(
                        fields
                            .take(8)
                            .map(|f| f.parse::<u64>().ok())
                            .sum::<Option<u64>>()?,
                    )
                }
                Some(name) if name.starts_with("cpu") => cores += 1,
                _ => {}
            }
        }
        Some(Self {
            process,
            total: total?,
            cores,
        })
    }

    /// CPU Usage between two readings, as a percentage of one core.
    pub(crate) fn usage_since(&self, earlier: &CpuReading) -> Option<f64> {
        let process = self.process.checked_sub(earlier.process)?;
        let total = self.total.checked_sub(earlier.total)?;
        if total == 0 || self.cores == 0 {
            return None;
        }
        let per_core = total as f64 / f64::from(self.cores);
        Some(process as f64 / per_core * 100.0)
    }
}

/// utime + stime from a `/proc/<pid>/stat` line.
fn process_ticks(line: &str) -> Option<u64> {
    // The process name is in parentheses and may contain spaces, so count
    // fields from after the closing one: state is field 3, utime 14, stime 15.
    let after_name = &line[line.rfind(')')? + 1..];
    let mut fields = after_name.split_whitespace().skip(11);
    let utime: u64 = fields.next()?.parse().ok()?;
    let stime: u64 = fields.next()?.parse().ok()?;
    Some(utime + stime)
}
