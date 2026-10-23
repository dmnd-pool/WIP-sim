//! Captured Firefox launch and bounded diagnostics shared by actual browser checks.
use crate::browser_groups;
use anyhow::{ensure, Context, Result};
use md_sim_access::browser::{PageLoad, Session};
use md_sim_runtime::{isolation::save_json, store::hash_file};
use serde_json::{json, Value};
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    os::unix::{fs::PermissionsExt, process::CommandExt},
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

// Actual isolated trace013 shows a 16GiB PROT_NONE reservation failing under
// the former 16GiB process limit. Leave room for aligned sandbox reservations
// and ordinary mappings. The cgroup still limits resident memory to768MiB;
// the enclosing VM has2GiB and its host process has a3GiB address-space cap.
pub const ADDRESS_SPACE_BYTES: u64 = 64 * 1024 * 1024 * 1024;
pub const CPU_SECONDS: u64 = 300;
pub const STARTUP_SECONDS: u64 = 300;
pub const NEW_SESSION_SECONDS: u64 = 270;
// Leave space for the supervisor and other owned support tools outside a
// browser leaf. Its cgroup separately enforces the lower256-task limit.
pub const UID_TASK_LIMIT: u64 = 512;

const PREFS: &str = r#"user_pref("remote.log.level", "Trace");
user_pref("browser.dom.window.dump.enabled", true);
user_pref("devtools.console.stdout.chrome", true);
user_pref("devtools.console.stdout.content", true);
user_pref("browser.shell.checkDefaultBrowser", false);
user_pref("browser.startup.homepage_override.mstone", "ignore");
user_pref("browser.startup.page", 0);
user_pref("browser.newtabpage.enabled", false);
user_pref("browser.newtabpage.activity-stream.feeds.telemetry", false);
user_pref("browser.newtabpage.activity-stream.telemetry", false);
user_pref("datareporting.policy.dataSubmissionEnabled", false);
user_pref("datareporting.healthreport.uploadEnabled", false);
user_pref("toolkit.telemetry.enabled", false);
user_pref("app.update.auto", false);
user_pref("extensions.update.enabled", false);
user_pref("network.captive-portal-service.enabled", false);
user_pref("network.connectivity-service.enabled", false);
user_pref("network.prefetch-next", false);
user_pref("network.dns.disablePrefetch", true);
user_pref("browser.safebrowsing.downloads.remote.enabled", false);
user_pref("signon.rememberSignons", false);
"#;

pub struct Browser(Child, browser_groups::Group);
impl Browser {
    /// Stop this leaf and reap its direct Firefox child before another leaf's
    /// orphan cleanup runs. This supports two live, isolated profiles.
    pub fn terminate(&mut self) -> Result<()> {
        self.1.kill()?;
        self.0.wait()?;
        Ok(())
    }
    pub fn stop(&mut self) -> Result<Value> {
        self.terminate()?;
        self.1.finish()
    }
}
impl Drop for Browser {
    fn drop(&mut self) {
        // The pre-created leaf remains ours for the whole VM run. It also
        // contains descendants which create a new process group or session.
        let _ = self.1.kill();
        let _ = self.0.wait();
        let _ = self.1.finish();
    }
}
fn exited(child: &Child) -> Result<bool> {
    let mut status: libc::siginfo_t = unsafe { std::mem::zeroed() };
    ensure!(
        unsafe {
            libc::waitid(
                libc::P_PID,
                child.id(),
                &mut status,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        } == 0,
        "browser wait observation failed"
    );
    Ok(unsafe { status.si_pid() } != 0)
}
pub fn bounded_log(path: &Path) -> Result<Value> {
    let file = File::open(path)?;
    let size = file.metadata()?.len();
    let mut bytes = Vec::new();
    file.take(65536).read_to_end(&mut bytes)?;
    let mut result = json!({"object":hash_file(path)?,"prefix_hex":hex::encode(bytes),"prefix_truncated":size>65536});
    if size > 65536 {
        let mut file = File::open(path)?;
        file.seek(SeekFrom::End(-65536))?;
        let mut tail = Vec::new();
        file.take(65536).read_to_end(&mut tail)?;
        result["suffix_hex"] = json!(hex::encode(tail));
    }
    Ok(result)
}
fn process_diagnostics() -> Result<Value> {
    let mut processes = Vec::new();
    for entry in fs::read_dir("/proc")? {
        let path = entry?.path();
        if !path.file_name().is_some_and(|n| {
            n.to_str()
                .is_some_and(|s| s.bytes().all(|b| b.is_ascii_digit()))
        }) {
            continue;
        }
        let status = match fs::read_to_string(path.join("status")) {
            Ok(s) => s,
            Err(_) => continue,
        };
        if !status.lines().any(|line| {
            line.starts_with("Uid:")
                && line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|uid| uid.parse::<u32>().ok())
                    == Some(browser_groups::UID)
        }) {
            continue;
        }
        let mut record = json!({"pid":path.file_name().map(|n|n.to_string_lossy().into_owned()),"status":status});
        for name in ["stat", "wchan", "cmdline"] {
            let mut bytes = Vec::new();
            if let Ok(file) = File::open(path.join(name)) {
                file.take(8192).read_to_end(&mut bytes)?;
            }
            record[name] = json!(String::from_utf8_lossy(&bytes).replace('\0', " "));
        }
        let mut threads = Vec::new();
        if let Ok(entries) = fs::read_dir(path.join("task")) {
            for entry in entries.take(browser_groups::TASK_LIMIT + 1) {
                let path = entry?.path();
                let mut thread =
                    json!({"tid":path.file_name().map(|n|n.to_string_lossy().into_owned())});
                for name in ["comm", "wchan", "stat", "syscall"] {
                    if let Ok(file) = File::open(path.join(name)) {
                        let mut bytes = Vec::new();
                        file.take(4096).read_to_end(&mut bytes)?;
                        thread[name] = json!(String::from_utf8_lossy(&bytes));
                    }
                }
                threads.push(thread);
            }
            ensure!(
                threads.len() <= browser_groups::TASK_LIMIT,
                "thread diagnostic bound exceeded"
            );
        }
        record["threads"] = json!(threads);
        processes.push(record);
        ensure!(
            processes.len() <= UID_TASK_LIMIT as usize,
            "browser process diagnostic bound exceeded"
        );
    }
    let mut profile_files = Vec::new();
    if let Ok(entries) = fs::read_dir("/work/browser/profile-0") {
        for entry in entries.take(257) {
            let entry = entry?;
            let info = entry.metadata()?;
            profile_files.push(json!({"name":entry.file_name().to_string_lossy(),"size":info.len(),"directory":info.is_dir()}));
        }
        ensure!(
            profile_files.len() <= 256,
            "profile inventory exceeds bound"
        );
    }
    Ok(
        json!({"processes":processes,"profile_files":profile_files,"memory":fs::read_to_string("/proc/meminfo")?,"entropy_available":fs::read_to_string("/proc/sys/kernel/random/entropy_avail")?}),
    )
}
pub fn prepare_profile(index: usize, additional_preferences: &str) -> Result<String> {
    ensure!(index < 2, "profile index exceeds bound");
    let profile = format!("/work/browser/profile-{index}");
    fs::create_dir(&profile)?;
    fs::set_permissions(&profile, fs::Permissions::from_mode(0o700))?;
    fs::write(
        format!("{profile}/user.js"),
        format!(
            "user_pref(\"marionette.port\", {});\n{PREFS}{additional_preferences}",
            2828 + index
        ),
    )?;
    Ok(profile)
}
fn spawn(
    index: usize,
    group: browser_groups::Group,
    arguments: &[&str],
    trace_memory: bool,
) -> Result<Browser> {
    ensure!(index < 2, "profile index exceeds bound");
    let profile = format!("/work/browser/profile-{index}");
    let attach_fd = group.attach_fd();
    let mut command = Command::new(if trace_memory {
        "/usr/bin/strace"
    } else {
        "/usr/lib/firefox/firefox"
    });
    if trace_memory {
        command.args([
            "-f",
            "-e",
            "trace=mmap,mprotect,prlimit64,clone,clone3,fork,vfork",
            "-e",
            "status=failed",
            "-o",
            "/work/browser/memory-syscalls.log",
            "--",
            "/usr/lib/firefox/firefox",
        ]);
    }
    command
        .args(["--headless", "--no-remote", "--profile", &profile])
        .args(arguments)
        .env_clear()
        .env("PATH", "/usr/bin")
        .env("MOZ_HEADLESS", "1")
        .env("TMPDIR", "/work/browser/tmp")
        .env("XDG_CACHE_HOME", "/work/browser/cache")
        .env("XDG_RUNTIME_DIR", "/work/browser/runtime")
        .env("XDG_CONFIG_HOME", "/work/browser/config")
        .env("FONTCONFIG_PATH", "/etc/fonts")
        .stdin(Stdio::null())
        .stdout(File::create(format!("/work/browser/{index}-stdout.log"))?)
        .stderr(File::create(format!("/work/browser/{index}-stderr.log"))?);
    if arguments.first() == Some(&"--screenshot") {
        command.env(
            "MOZ_LOG",
            "timestamp,sync,nsJarProtocol:5,DocLoader:5,nsDocLoader:5,XMLContentSink:5,expatdriver:5,DocumentChannel:5,console:5",
        );
    }
    let expected_parent = std::process::id() as libc::pid_t;
    unsafe {
        command.pre_exec(move || {
            if libc::write(attach_fd, b"0\n".as_ptr().cast(), 2) != 2 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::setsid() < 0 || libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::getppid() != expected_parent {
                return Err(std::io::Error::from_raw_os_error(libc::ESRCH));
            }
            for (resource, value) in [
                (libc::RLIMIT_CPU, CPU_SECONDS),
                (libc::RLIMIT_AS, ADDRESS_SPACE_BYTES),
                (libc::RLIMIT_FSIZE, 64 * 1024 * 1024),
                (libc::RLIMIT_NPROC, UID_TASK_LIMIT),
                (libc::RLIMIT_NOFILE, 1024),
                (libc::RLIMIT_CORE, 0),
            ] {
                let limit = libc::rlimit {
                    rlim_cur: value,
                    rlim_max: value,
                };
                if libc::setrlimit(resource, &limit) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    Ok(Browser(command.spawn()?, group))
}
pub fn launch(index: usize, group: browser_groups::Group) -> Result<(Browser, Session, Value)> {
    launch_with_trace(index, group, false)
}
pub fn launch_with_trace(
    index: usize,
    group: browser_groups::Group,
    trace_syscalls: bool,
) -> Result<(Browser, Session, Value)> {
    launch_policy(index, group, trace_syscalls, PageLoad::Complete)
}

#[allow(dead_code)] // Used by the native S9 suite, not generic browser probes.
pub fn launch_interactive(
    index: usize,
    group: browser_groups::Group,
) -> Result<(Browser, Session, Value)> {
    launch_policy(index, group, false, PageLoad::Interactive)
}

fn launch_policy(
    index: usize,
    group: browser_groups::Group,
    trace_syscalls: bool,
    page_load: PageLoad,
) -> Result<(Browser, Session, Value)> {
    let profile = format!("/work/browser/profile-{index}");
    let browser = spawn(
        index,
        group,
        &["--marionette", "about:blank"],
        trace_syscalls,
    )?;
    let start = Instant::now();
    let mut last_error = String::new();
    // This is the support browser under software CPU emulation, not a guest
    // miner boot deadline. Probes016/018 still had a running UI main thread
    // when the120-second session deadline expired. Keep a bounded longer
    // observation; all DOM, live-service and profile checks remain required.
    let startup_budget = Duration::from_secs(STARTUP_SECONDS);
    while start.elapsed() < startup_budget {
        ensure!(
            !exited(&browser.0)?,
            "browser exited before control readiness; last error {last_error}"
        );
        match Session::connect(
            format!("127.0.0.1:{}", 2828 + index).parse()?,
            Duration::from_secs(2),
        ) {
            Ok(mut session) => {
                // Startup uses a short connection timeout; each real browser
                // command has an independent, absolute60-second deadline.
                // Trace014 showed real UI loading still active at60seconds.
                session.set_timeout(Duration::from_secs(60))?;
                let remaining = startup_budget
                    .checked_sub(start.elapsed())
                    .context("browser startup budget exhausted")?;
                let capabilities = session
                    .start_with_page_load(
                        remaining.min(Duration::from_secs(NEW_SESSION_SECONDS)),
                        page_load,
                    )
                    .inspect_err(|_| {
                        if let Ok(diagnostics) = process_diagnostics() {
                            let _ = save_json(
                                Path::new("/work/browser/start-failure.json"),
                                &diagnostics,
                            );
                        }
                    })
                    .context("Firefox NewSession did not complete")?;
                ensure!(
                    capabilities["capabilities"]["acceptInsecureCerts"] == false,
                    "browser did not retain certificate verification"
                );
                ensure!(
                    capabilities["capabilities"]["pageLoadStrategy"] == page_load.capability(),
                    "browser navigation policy differs"
                );
                ensure!(
                    capabilities["capabilities"]["moz:profile"] == profile,
                    "browser control connection selected another profile"
                );
                return Ok((
                    browser,
                    session,
                    json!({"profile":profile,"capabilities":capabilities,"startup_millis":start.elapsed().as_millis(),"prefs":hash_file(Path::new(&format!("{profile}/user.js")))?}),
                ));
            }
            Err(error) => last_error = format!("{error:#}"),
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    anyhow::bail!("browser control startup deadline: {last_error}")
}

/// Separate renderer diagnostic. A screenshot cannot establish Marionette,
/// native dashboard, firmware, or mining acceptance.
pub fn render(group: browser_groups::Group, trace_memory: bool) -> Result<Value> {
    prepare_profile(0, "")?;
    let html = "<!doctype html><html><body style='background:white;color:black'><h1>SIMULATOR BROWSER DIAGNOSTIC</h1><p id='result'>Pending</p><script>document.querySelector('#result').textContent='Actual page script executed';</script></body></html>";
    // A literal # starts the URL fragment and truncates the HTML payload.
    let url = format!(
        "data:text/html,{}",
        html.as_bytes()
            .iter()
            .map(|byte| format!("%{byte:02X}"))
            .collect::<String>()
    );
    let mut browser = spawn(
        0,
        group,
        &[
            "--screenshot",
            "/work/browser/render.png",
            "--window-size",
            "800,600",
            &url,
        ],
        trace_memory,
    )?;
    let start = Instant::now();
    let result = (|| -> Result<Value> {
        while !exited(&browser.0)? {
            ensure!(
                start.elapsed() < Duration::from_secs(90),
                "headless render deadline"
            );
            std::thread::sleep(Duration::from_millis(100));
        }
        let status = browser.0.wait()?;
        ensure!(status.success(), "headless renderer failed: {status}");
        let mut bytes = Vec::new();
        File::open("/work/browser/render.png")?
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() <= 1024 * 1024
                && bytes.len() >= 33
                && bytes[..8] == *b"\x89PNG\r\n\x1a\n"
                && bytes[12..16] == *b"IHDR"
                && bytes[16..20] == 800u32.to_be_bytes()
                && bytes[20..24] == 600u32.to_be_bytes(),
            "rendered PNG header differs"
        );
        Ok(
            json!({"passed":true,"elapsed_millis":start.elapsed().as_millis(),
            "image":hash_file(Path::new("/work/browser/render.png"))?,"image_hex":hex::encode(bytes),
            "source_url":url}),
        )
    })();
    if result.is_err() {
        save_json(
            Path::new("/work/browser/start-failure.json"),
            &process_diagnostics()?,
        )?;
    }
    let cleanup = browser.stop()?;
    drop(browser);
    result.map(|mut value| {
        value["cleanup"] = cleanup;
        value
    })
}
