use crate::{Error, Result};

/// Get absolute number of CPUs, including isolated (Linux only)
#[cfg(target_os = "linux")]
pub fn num_cpus() -> Result<usize> {
    use std::io::BufRead;

    let f = std::fs::File::open("/proc/cpuinfo")?;
    let reader = std::io::BufReader::new(f);
    let lines = reader.lines();
    let mut count = 0;
    for line in lines {
        let line = line?;
        if line
            .split(':')
            .next()
            .ok_or_else(|| Error::Failed("invalid line".into()))?
            .trim_end()
            == "processor"
        {
            count += 1;
        }
    }
    Ok(count)
}

/// Get absolute number of CPUs, including isolated (Linux only)
#[cfg(not(target_os = "linux"))]
pub fn num_cpus() -> Result<usize> {
    Err(Error::Unimplemented)
}

/// Linux-specific system tools
pub mod linux {
    use crate::Result;

    use core::fmt;
    use std::{collections::BTreeMap, fs};
    use tracing::warn;

    /// Configure system parameters (global) while the process is running. Does nothing in simulated
    /// mode
    ///
    /// Example:
    ///
    /// ```rust,no_run
    /// use rtsc::system::linux::SystemConfig;
    ///
    /// let _sys = SystemConfig::new().set("kernel/sched_rt_runtime_us", -1)
    ///     .apply()
    ///     .expect("Unable to set system config");
    /// // some code
    /// // system config is restored at the end of the scope
    /// ```
    #[derive(Default)]
    pub struct SystemConfig {
        values: BTreeMap<&'static str, String>,
        prev_values: BTreeMap<&'static str, String>,
    }

    impl SystemConfig {
        /// Creates a new system config object
        #[must_use]
        pub fn new() -> Self {
            Self::default()
        }
        /// Set a parameter to configure
        pub fn set<V: fmt::Display>(mut self, key: &'static str, value: V) -> Self {
            self.values.insert(key, value.to_string());
            self
        }
        /// Apply values to /proc/sys keys
        pub fn apply(mut self) -> Result<SystemConfigGuard> {
            for (key, value) in &self.values {
                let fname = format!("/proc/sys/{}", key);
                let prev_value = fs::read_to_string(&fname)?;
                self.prev_values.insert(key, prev_value);
                fs::write(fname, value)?;
            }
            Ok(SystemConfigGuard { config: self })
        }
    }

    /// A guard object to restore system parameters when dropped
    #[derive(Default)]
    pub struct SystemConfigGuard {
        config: SystemConfig,
    }

    impl Drop for SystemConfigGuard {
        fn drop(&mut self) {
            for (key, value) in &self.config.prev_values {
                if let Err(error) = fs::write(format!("/proc/sys/{}", key), value) {
                    warn!(%key, %value, %error, "Failed to restore system config");
                }
            }
        }
    }

    /// Configure CPU governors for the given CPUs
    #[derive(Default)]
    pub struct CpuGovernor {
        prev_governor: BTreeMap<usize, String>,
    }

    impl CpuGovernor {
        /// Set performance governor for the given CPUs. This sets the maximum frequency for the
        /// CPUs, increasing the power consumption but lowering their latency. It is enough to
        /// specify a single logical core number per physical core. The governor is restored when
        /// the returned guard object is dropped.
        pub fn performance<I>(performance_cpus: I) -> Result<CpuGovernor>
        where
            I: IntoIterator<Item = usize>,
        {
            let mut prev_governor = BTreeMap::new();
            for cpu in performance_cpus {
                let fname = format!(
                    "/sys/devices/system/cpu/cpu{}/cpufreq/scaling_governor",
                    cpu
                );
                let prev_value = fs::read_to_string(fname)?;
                prev_governor.insert(cpu, prev_value.trim().to_string());
            }
            for cpu in prev_governor.keys() {
                let fname = format!(
                    "/sys/devices/system/cpu/cpu{}/cpufreq/scaling_governor",
                    cpu
                );
                fs::write(fname, "performance")?;
            }
            Ok(CpuGovernor { prev_governor })
        }
    }

    impl Drop for CpuGovernor {
        fn drop(&mut self) {
            for (cpu, governor) in &self.prev_governor {
                if let Err(error) = fs::write(
                    format!(
                        "/sys/devices/system/cpu/cpu{}/cpufreq/scaling_governor",
                        cpu
                    ),
                    governor,
                ) {
                    warn!(cpu, %error, "Failed to restore governor");
                }
            }
        }
    }
}
