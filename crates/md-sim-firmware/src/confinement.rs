//! Validate the contract before any acquired code executes.
use anyhow::{ensure, Context, Result};
use std::{fs, path::Path};
pub(crate) fn require(preparation: bool) -> Result<()> {
    require_with_memory_limit(
        preparation,
        if preparation {
            2 * 1024 * 1024 * 1024
        } else {
            1024 * 1024 * 1024
        },
        if preparation { 120 } else { 100 },
        if preparation {
            64 * 1024 * 1024
        } else {
            24 * 1024 * 1024
        },
    )
}

/// Existing FPGA converter builds maps for3.61 million physical wire instances
/// and an in-memory SQLite graph. This preparation-only budget leaves ordinary
/// firmware and export limits unchanged; all namespace/mount checks still apply.
#[allow(dead_code)]
pub(crate) fn require_netlist_preparation() -> Result<()> {
    require_with_memory_limit(true, 4 << 30, 600, 1 << 30)
}

/// Compile the pinned existing HDL toolchain. Two compiler jobs share a closed
/// preparation namespace. The600-second CPU budget does not change guest time.
#[allow(dead_code)]
pub(crate) fn require_hdl_tool_build() -> Result<()> {
    require_with_memory_limit(true, 3 << 30, 600, 128 << 20)
}

/// Read-only filesystem inspection needs room for an expanded firmware disk.
/// Acquired programs remain data; only captured host analysis tools execute.
#[allow(dead_code)]
pub(crate) fn require_firmware_inspection() -> Result<()> {
    require_with_memory_limit(true, 512 << 20, 30, 128 << 20)
}

/// QEMU ARM user mode reserves the full32-bit guest range, including its
/// high helper page. This is virtual space, not a6GiB resident-memory claim.
#[allow(dead_code)]
pub(crate) fn require_arm_protocol_oracle() -> Result<()> {
    require_with_memory_limit(true, 6 * 1024 * 1024 * 1024, 60, 64 << 20)
}

/// Full-system emulation reserves guest RAM plus the backend's address space.
/// Keep the namespace, file, CPU and process checks of bounded preparation.
#[allow(dead_code)] // Also included directly by standalone example executables.
pub(crate) fn require_board() -> Result<()> {
    require_with_memory_limit(true, 3 * 1024 * 1024 * 1024, 120, 64 * 1024 * 1024)
}

/// A native worker and client VM share a closed support namespace. Its
/// nonzero identity permits ordinary nested user mapping without retaining
/// parent capabilities. The worker's jobs use their usual separate contract.
#[allow(dead_code)]
pub(crate) fn require_native_worker_host() -> Result<()> {
    require_with_identity(true, 3 << 30, 120, 64 << 20, 1000)?;
    require_native_worker_privileges()
}

/// Explicit support allowance for a complete captured miner bundle. Each job
/// still has its original smaller process limits and clock policy.
#[allow(dead_code)]
pub(crate) fn require_native_firmware_worker_host() -> Result<()> {
    require_with_identity(true, 4 << 30, 900, 256 << 20, 1000)?;
    require_native_worker_privileges()
}

/// Browser payload, two bounded Firefox processes, and artifact retention in
/// the support VM. The separately executed native job keeps its own limits.
#[allow(dead_code)]
pub(crate) fn require_native_firmware_browser_worker_host() -> Result<()> {
    require_with_identity(true, 6 << 30, 2000, 256 << 20, 1000)?;
    require_native_worker_privileges()
}

/// Only the trusted x86 support launcher can request the runtime's KVM grant.
/// The native miner runs in its own ordinary process namespace without KVM.
#[allow(dead_code)]
pub(crate) fn require_native_browser_kvm_host() -> Result<()> {
    require_with_identity_capability(true, 6 << 30, 2000, 256 << 20, 1000, true)?;
    require_native_worker_privileges()
}

#[allow(dead_code)]
pub(crate) fn require_kvm_prerequisite() -> Result<()> {
    require_with_identity_capability(true, 3 << 30, 120, 64 << 20, 0, true)
}

/// A private client image can contain a captured firmware object above64MiB.
/// The image builder separately bounds the complete uncompressed archive.
#[allow(dead_code)]
pub(crate) fn require_private_vm_preparation() -> Result<()> {
    require_with_memory_limit(true, 2 << 30, 120, 128 << 20)
}

fn require_native_worker_privileges() -> Result<()> {
    let status = fs::read_to_string("/proc/self/status")?;
    for key in ["CapPrm:", "CapEff:", "CapBnd:"] {
        let value = status
            .lines()
            .find_map(|line| line.strip_prefix(key))
            .context("worker capability field absent")?;
        ensure!(
            u64::from_str_radix(value.trim(), 16)? == 0,
            "native worker must retain no capabilities"
        );
    }
    ensure!(
        status
            .lines()
            .find_map(|line| line.strip_prefix("NoNewPrivs:"))
            .map(str::trim)
            == Some("1")
            && unsafe { libc::getegid() } == 1000,
        "native worker privilege or group identity differs"
    );
    Ok(())
}

/// Original boot plus mining services. S9 pool015 exhausted110 CPU seconds;
/// pool031 reached230 CPU seconds before its console/storage suite finished.
/// This profile permits at most360 CPU seconds. Each run records its smaller
/// selected allowance; namespace, memory and file bounds stay explicit.
#[allow(dead_code)]
pub(crate) fn require_mining_board() -> Result<()> {
    require_with_memory_limit(true, 3 * 1024 * 1024 * 1024, 360, 64 * 1024 * 1024)
}

#[allow(dead_code)]
pub(crate) const MINING_PROFILE_ENVIRONMENT: &str = "MD_SIM_MINING_EXECUTION_PROFILE";
#[allow(dead_code)]
pub(crate) const NESTED_MINING_WALL_SECONDS: u64 = 1200;
#[allow(dead_code)]
pub(crate) const NESTED_MINING_LONG_WALL_SECONDS: u64 = 1800;
#[allow(dead_code)]
pub(crate) const NESTED_MINING_SLOW_WALL_SECONDS: u64 = 2400;
#[allow(dead_code)]
pub(crate) const NESTED_MINING_LONG_BOOT_SECONDS: u64 = 4800;

#[allow(dead_code)]
pub(crate) fn mining_profile_wall(value: Option<&str>) -> Result<Option<u64>> {
    match value {
        None => Ok(None),
        Some("nested-tcg") => Ok(Some(NESTED_MINING_WALL_SECONDS)),
        Some("nested-tcg-1800") => Ok(Some(NESTED_MINING_LONG_WALL_SECONDS)),
        Some("nested-tcg-2400") => Ok(Some(NESTED_MINING_SLOW_WALL_SECONDS)),
        Some("nested-tcg-4800") => Ok(Some(NESTED_MINING_LONG_BOOT_SECONDS)),
        Some("nested-tcg-4800-two-cpu") => Ok(Some(NESTED_MINING_LONG_BOOT_SECONDS)),
        _ => anyhow::bail!("unsupported mining execution profile"),
    }
}

#[allow(dead_code)]
pub(crate) fn mining_profile_cpu(value: Option<&str>) -> Result<Option<u64>> {
    Ok(mining_profile_wall(value)?.map(|wall| {
        if value == Some("nested-tcg-4800-two-cpu") {
            2 * wall - 10
        } else {
            wall - 10
        }
    }))
}

#[allow(dead_code)]
pub(crate) fn nested_mining_wall_seconds() -> Result<Option<u64>> {
    let value = std::env::var(MINING_PROFILE_ENVIRONMENT);
    match value {
        Err(std::env::VarError::NotPresent) => mining_profile_wall(None),
        Ok(value) => mining_profile_wall(Some(&value)),
        _ => anyhow::bail!("invalid mining execution profile encoding"),
    }
}

#[allow(dead_code)]
pub(crate) fn nested_mining_profile() -> Result<bool> {
    Ok(nested_mining_wall_seconds()?.is_some())
}

/// SSH pipe120 spent336s of emulator CPU reaching native procd early startup.
/// This explicit resource profile permits more instruction translation work.
/// It changes no guest clock, bus timing, mining filter or boot assertion.
/// SSH134 with an instruction clock progressed through native S99 startup
/// before its1200-second host limit. The separate1800 profile leaves time for
/// that measured startup and the unchanged230-second mining window.
/// Both143 runs reached ASIC dummy work only after1730s of nested execution.
/// The separate2400 profile permits that measured startup plus a declared
/// longer pool observation window. Device clocks and firmware stay unchanged.
/// Realtime179 exhausted2400s: native services had only just started and no
/// pool session existed. The distinct4800s profile leaves an explicit boot
/// allowance before the same bounded900s pool window. It changes no guest timer.
/// Local200 ended with SIGKILL near its4790s CPU ceiling, but cumulative
/// child accounting could not prove the cause. The separate two-cpu profile
/// retains4800s wall and permits9590s CPU for two guest CPUs and support threads.
/// Its distinct identity must be tested; no service or boot pass is inferred.
#[allow(dead_code)]
pub(crate) fn require_mining_execution() -> Result<()> {
    let value = match std::env::var(MINING_PROFILE_ENVIRONMENT) {
        Err(std::env::VarError::NotPresent) => None,
        Ok(value) => Some(value),
        _ => anyhow::bail!("invalid mining execution profile encoding"),
    };
    if let Some(cpu) = mining_profile_cpu(value.as_deref())? {
        require_with_memory_limit(true, 3 * 1024 * 1024 * 1024, cpu, 64 * 1024 * 1024)
    } else {
        require_mining_board()
    }
}

/// The captured support browser starts before the miner. Its parent lowers
/// the CPU and address limits before miner code and keeps a separate wall guard.
#[allow(dead_code)]
pub(crate) fn require_mining_browser_board() -> Result<()> {
    require_with_memory_limit(true, 4 * 1024 * 1024 * 1024, 600, 64 * 1024 * 1024)
}

/// Combined Rust runtime and UBSan linking exceeded64MiB before stripping.
/// This allowance applies only to emulator preparation, not firmware runs.
#[allow(dead_code)]
pub(crate) const EMULATOR_BUILD_FILE_BYTES: u64 = 128 * 1024 * 1024;

#[allow(dead_code)]
pub(crate) fn require_emulator_build() -> Result<()> {
    require_with_memory_limit(true, 3 * 1024 * 1024 * 1024, 120, EMULATOR_BUILD_FILE_BYTES)
}

/// A disconnected full-system guest that runs another bounded emulator.
/// The outer backend pays the CPU cost of both instruction translators.
#[allow(dead_code)]
pub(crate) fn require_nested_board() -> Result<()> {
    require_with_memory_limit(true, 3 * 1024 * 1024 * 1024, 300, 64 * 1024 * 1024)
}

/// Complete native service suites pay the cost of nested instruction translation.
/// This distinct profile follows measured inner CPU exhaustion, not guest timing changes.
#[allow(dead_code)]
pub(crate) fn require_native_dashboard_vm() -> Result<()> {
    require_with_memory_limit(true, 3 * 1024 * 1024 * 1024, 480, 64 * 1024 * 1024)
}

/// Two fresh support-browser profiles under software CPU emulation. This
/// profile does not change the limits or clocks of a miner execution.
#[allow(dead_code)]
pub(crate) fn require_browser_vm() -> Result<()> {
    require_with_memory_limit(true, 3 * 1024 * 1024 * 1024, 720, 64 * 1024 * 1024)
}

/// Captured process execution in Linux, with separate inner process limits.
#[allow(dead_code)]
pub(crate) fn require_process_vm() -> Result<()> {
    require_with_memory_limit(true, 5 * 1024 * 1024 * 1024, 1440, 64 * 1024 * 1024)
}

/// The enclosing VM pays for nested software instruction translation plus
/// authenticated transfer. The inner process retains its own smaller limits.
#[allow(dead_code)]
pub(crate) fn require_extended_process_vm() -> Result<()> {
    require_with_memory_limit(true, 5 * 1024 * 1024 * 1024, 3600, 64 * 1024 * 1024)
}

#[allow(dead_code)]
pub(crate) fn require_long_process_vm() -> Result<()> {
    require_with_memory_limit(true, 5 * 1024 * 1024 * 1024, 4800, 64 * 1024 * 1024)
}

#[allow(dead_code)]
pub(crate) fn require_slow_process_vm() -> Result<()> {
    require_with_memory_limit(true, 5 * 1024 * 1024 * 1024, 6000, 64 * 1024 * 1024)
}

/// Captured worker inputs, nested1GiB firmware, and two private Firefox trees.
/// SSH163 exhausted its3GiB guest and the kernel killed QEMU and Firefox.
/// Four support vCPUs have a bounded3000s window. Inner limits are unchanged.
#[allow(dead_code)]
pub(crate) fn require_board_browser_vm() -> Result<()> {
    require_with_memory_limit(true, 8 * 1024 * 1024 * 1024, 12000, 64 * 1024 * 1024)
}

/// Explicit support bounds for the4800s inner boot profile and transfer/browser
/// overhead. Four browser vCPUs or two non-browser vCPUs retain separate limits.
#[allow(dead_code)]
pub(crate) fn require_long_boot_process_vm(browser: bool) -> Result<()> {
    require_with_memory_limit(
        true,
        (if browser { 8 } else { 5 }) * 1024 * 1024 * 1024,
        if browser { 21600 } else { 10800 },
        64 * 1024 * 1024,
    )
}

#[allow(dead_code)]
// The reboot suite now runs two nested boots plus both browser profiles.
// Allow the two outer vCPUs to run for the bounded wall interval. Native023
// received SIGKILL under the previous1260s cap; its CPU usage was not saved,
// so that signal alone is not claimed as proof of CPU-limit exhaustion.
pub(crate) const NATIVE_BROWSER_VM_CPU_SECONDS: u64 = 2880;
#[allow(dead_code)]
pub(crate) const NATIVE_BROWSER_VM_WALL_SECONDS: u64 = 1440;
#[allow(dead_code)]
pub(crate) const NATIVE_BROWSER_VM_ADDRESS_BYTES: u64 = 6 * 1024 * 1024 * 1024;
/// Combined native HTTP/RPC and two fresh browser profiles. The measured
/// separate suites used332s and613s. This is a distinct support-tool profile.
#[allow(dead_code)]
pub(crate) fn require_native_browser_vm() -> Result<()> {
    require_with_memory_limit(
        true,
        NATIVE_BROWSER_VM_ADDRESS_BYTES,
        NATIVE_BROWSER_VM_CPU_SECONDS,
        64 * 1024 * 1024,
    )
}

/// Complete NAND data and spare storage stays in the private run tmpfs.
#[allow(dead_code)]
pub(crate) fn require_nand_storage() -> Result<()> {
    require_with_memory_limit(false, 1024 * 1024 * 1024, 20, 300 * 1024 * 1024)
}

fn require_with_memory_limit(
    preparation: bool,
    address_limit: u64,
    cpu_limit: u64,
    file_limit: u64,
) -> Result<()> {
    require_with_identity(preparation, address_limit, cpu_limit, file_limit, 0)
}

fn require_with_identity(
    preparation: bool,
    address_limit: u64,
    cpu_limit: u64,
    file_limit: u64,
    namespace_uid: u32,
) -> Result<()> {
    require_with_identity_capability(
        preparation,
        address_limit,
        cpu_limit,
        file_limit,
        namespace_uid,
        false,
    )
}

fn require_with_identity_capability(
    preparation: bool,
    address_limit: u64,
    cpu_limit: u64,
    file_limit: u64,
    namespace_uid: u32,
    kvm: bool,
) -> Result<()> {
    let status = fs::read_to_string("/proc/self/stat")?;
    let proc_pid: u32 = status
        .split_whitespace()
        .next()
        .context("proc PID absent")?
        .parse()?;
    ensure!(
        proc_pid == std::process::id(),
        "proc filesystem belongs to another PID namespace"
    );
    let map = fs::read_to_string("/proc/self/uid_map")?;
    let fields: Vec<_> = map.split_whitespace().collect();
    ensure!(
        fields.len() == 3 && fields[0] == namespace_uid.to_string()
            // Bubblewrap can use an intermediate root namespace to construct
            // a nonroot sandbox. Parent IDs are relative, not host IDs. Keep
            // the original root-guest restriction; the support worker instead
            // requires nonzero effective UID and zero retained capabilities.
            && (namespace_uid != 0 || fields[1] != "0") && fields[2] == "1"
            && unsafe { libc::geteuid() } == namespace_uid,
        "private single-user namespace identity differs"
    );
    let mountinfo = fs::read_to_string("/proc/self/mountinfo")?;
    let kvm_mounts = mountinfo
        .lines()
        .filter(|line| line.split_whitespace().nth(4) == Some("/dev/kvm"))
        .count();
    ensure!(
        kvm_mounts == usize::from(kvm),
        "KVM mount differs from support grant"
    );
    if kvm {
        use std::os::unix::fs::{FileTypeExt, MetadataExt};
        let device = fs::symlink_metadata("/dev/kvm")?;
        ensure!(
            device.file_type().is_char_device()
                && libc::major(device.rdev()) == 10
                && libc::minor(device.rdev()) == 232,
            "support KVM device identity differs"
        );
    }
    let bridge = md_sim_runtime::dashboard::guest_mount_expected()?;
    let bridge_mounts = mountinfo
        .lines()
        .filter(|line| {
            line.split_whitespace().nth(4) == Some(md_sim_runtime::dashboard::SOCKET_PATH)
        })
        .count();
    ensure!(
        bridge_mounts == usize::from(bridge),
        "dashboard mount and runtime environment differ"
    );
    let proc_mounts: Vec<_> = mountinfo
        .lines()
        .filter_map(|line| line.split_once(" - "))
        .map(|(left, _)| left.split_whitespace().collect::<Vec<_>>())
        .filter(|fields| fields.get(4) == Some(&"/proc"))
        .collect();
    ensure!(proc_mounts.len() == 1, "one private proc root is required");
    for line in mountinfo.lines() {
        let (left, right) = line.split_once(" - ").context("invalid mount metadata")?;
        let fields: Vec<_> = left.split_whitespace().collect();
        ensure!(fields.len() >= 6, "invalid mount fields");
        let path = fields[4];
        let filesystem = right
            .split_whitespace()
            .next()
            .context("missing filesystem type")?;
        match path {
            "/" => ensure!(
                filesystem == "tmpfs" && fields[5].split(',').any(|f| f == "ro"),
                "root must be read-only temporary storage"
            ),
            "/usr" if preparation => ensure!(
                fields[5].split(',').any(|f| f == "ro"),
                "build toolchain must be read-only"
            ),
            "/work" | "/tmp" | "/dev" => {
                ensure!(filesystem == "tmpfs", "scratch storage must be tmpfs")
            }
            "/proc" => ensure!(filesystem == "proc", "private proc mount required"),
            "/proc/irq" | "/proc/bus" | "/proc/sysrq-trigger" => {
                validate_proc_protection(&fields, filesystem, &proc_mounts[0])?;
            }
            "/dev/pts" => ensure!(filesystem == "devpts", "private PTY mount required"),
            "/dev/kvm" if kvm => {}
            md_sim_runtime::dashboard::SOCKET_PATH => {
                ensure!(
                    bridge
                        && fields[5].split(',').any(|flag| flag == "ro")
                        && !fields[5].split(',').any(|flag| flag == "rw"),
                    "dashboard socket must be the declared read-only runtime mount"
                );
            }
            "/dev/null" | "/dev/zero" | "/dev/full" | "/dev/random" | "/dev/urandom"
            | "/dev/tty" => {}
            _ => {
                ensure!(
                    Path::new(path).is_file() && fields[5].split(',').any(|flag| flag == "ro"),
                    "only read-only input file mounts are permitted: {path}"
                );
            }
        }
    }
    for line in fs::read_to_string("/proc/net/dev")?.lines().skip(2) {
        ensure!(
            line.split(':').next().map(str::trim) == Some("lo"),
            "external network interface is visible"
        );
    }
    for (resource, max) in [
        (libc::RLIMIT_CPU, cpu_limit),
        (libc::RLIMIT_AS, address_limit),
        (libc::RLIMIT_NPROC, if preparation { 256 } else { 128 }),
        (libc::RLIMIT_FSIZE, file_limit),
    ] {
        let mut limit = std::mem::MaybeUninit::<libc::rlimit>::uninit();
        ensure!(
            unsafe { libc::getrlimit(resource, limit.as_mut_ptr()) } == 0,
            "cannot read resource limit"
        );
        ensure!(
            unsafe { limit.assume_init() }.rlim_max <= max,
            "worker resource limit is absent or too large"
        );
    }
    Ok(())
}

// Bubblewrap creates these read-only proc submounts for the nonzero worker
// UID. Measured in network-vm/worker-mount-probe-117. They must belong to the
// same private proc mount, keep their exact root, and retain all protections.
fn validate_proc_protection(fields: &[&str], filesystem: &str, proc_root: &[&str]) -> Result<()> {
    ensure!(
        fields.len() >= 6 && proc_root.len() >= 6,
        "invalid protected proc fields"
    );
    ensure!(
        matches!(fields[4], "/proc/irq" | "/proc/bus" | "/proc/sysrq-trigger")
            && filesystem == "proc"
            && fields[1] == proc_root[0]
            && fields[2] == proc_root[2]
            && fields[3]
                == fields[4]
                    .strip_prefix("/proc")
                    .context("protected proc path absent")?
            && ["ro", "nosuid", "nodev", "noexec"]
                .iter()
                .all(|required| fields[5].split(',').any(|flag| flag == *required))
            && !fields[5].split(',').any(|flag| flag == "rw"),
        "proc protection is writable, foreign or has an unexpected root"
    );
    Ok(())
}

/// Compile and execute the explicit scalar ADC arithmetic adapter only.
/// Keep the standard private preparation namespaces and read-only inputs.
#[allow(dead_code)]
pub(crate) fn require_adc_reference_oracle() -> Result<()> {
    require_with_memory_limit(true, 2 << 30, 60, 2 << 20)?;
    let mut limit = std::mem::MaybeUninit::<libc::rlimit>::uninit();
    ensure!(
        unsafe { libc::getrlimit(libc::RLIMIT_NPROC, limit.as_mut_ptr()) } == 0,
        "cannot read oracle process limit"
    );
    ensure!(
        unsafe { limit.assume_init() }.rlim_max <= 16,
        "oracle process limit exceeds16"
    );
    Ok(())
}

#[cfg(test)]
mod proc_protection_tests {
    #[test]
    fn execution_profile_requires_an_explicit_bounded_budget() {
        assert_eq!(super::mining_profile_wall(None).unwrap(), None);
        assert_eq!(
            super::mining_profile_wall(Some("nested-tcg")).unwrap(),
            Some(1200)
        );
        assert_eq!(
            super::mining_profile_wall(Some("nested-tcg-1800")).unwrap(),
            Some(1800)
        );
        assert_eq!(
            super::mining_profile_wall(Some("nested-tcg-2400")).unwrap(),
            Some(2400)
        );
        assert_eq!(
            super::mining_profile_wall(Some("nested-tcg-4800")).unwrap(),
            Some(4800)
        );
        assert_eq!(
            super::mining_profile_wall(Some("nested-tcg-4800-two-cpu")).unwrap(),
            Some(4800)
        );
        for (profile, cpu) in [
            ("nested-tcg", 1190),
            ("nested-tcg-1800", 1790),
            ("nested-tcg-2400", 2390),
            ("nested-tcg-4800", 4790),
            ("nested-tcg-4800-two-cpu", 9590),
        ] {
            assert_eq!(super::mining_profile_cpu(Some(profile)).unwrap(), Some(cpu));
        }
        assert_eq!(super::mining_profile_cpu(None).unwrap(), None);
        for invalid in ["", "nested-tcg-0", "nested-tcg-3600", "1800", "standard"] {
            assert!(super::mining_profile_wall(Some(invalid)).is_err());
            assert!(super::mining_profile_cpu(Some(invalid)).is_err());
        }
    }
    use super::*;
    #[test]
    fn measured_worker_proc_protections_require_same_private_root_and_all_flags() {
        let root = [
            "188",
            "183",
            "0:33",
            "/",
            "/proc",
            "rw,nosuid,nodev,noexec,relatime",
        ];
        let measured = [
            "190",
            "188",
            "0:33",
            "/irq",
            "/proc/irq",
            "ro,nosuid,nodev,noexec,relatime",
        ];
        assert!(validate_proc_protection(&measured, "proc", &root).is_ok());
        for (index, invalid) in [
            (1, "187"),
            (2, "0:34"),
            (3, "/"),
            (4, "/proc/other"),
            (5, "rw,nosuid,nodev,noexec"),
            (5, "ro,nodev,noexec"),
            (5, "ro,nosuid,noexec"),
            (5, "ro,nosuid,nodev"),
        ] {
            let mut bad = measured;
            bad[index] = invalid;
            assert!(validate_proc_protection(&bad, "proc", &root).is_err());
        }
        assert!(validate_proc_protection(&measured, "tmpfs", &root).is_err());
        for (path, name) in [
            ("/sysrq-trigger", "/proc/sysrq-trigger"),
            ("/bus", "/proc/bus"),
        ] {
            let mut row = measured;
            row[3] = path;
            row[4] = name;
            assert!(validate_proc_protection(&row, "proc", &root).is_ok());
        }
    }
}
