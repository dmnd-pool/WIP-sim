//! Validate the already isolated server network before the independent pool
//! binds its fixed address. Network creation belongs to the support supervisor.
use super::shared_pool;
use anyhow::{ensure, Context, Result};
use md_sim_runtime::store::hash_file;
use serde_json::{json, Value};
use std::{
    fs,
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
    process::Command,
};

pub fn cancelled() -> Result<bool> {
    read_cancel(Path::new("/work/cancel"), 1000)
}

fn read_cancel(path: &Path, owner: u32) -> Result<bool> {
    let file = match fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file()
            && metadata.nlink() == 1
            && metadata.len() == 7
            && metadata.uid() == owner
            && metadata.mode() & 0o077 == 0,
        "unsafe shared pool cancellation signal"
    );
    let mut bytes = Vec::new();
    file.take(8).read_to_end(&mut bytes)?;
    ensure!(
        bytes == b"cancel\n",
        "shared pool cancellation signal differs"
    );
    Ok(true)
}

fn inspect(arguments: &[&str]) -> Result<Value> {
    let output = Command::new("/usr/bin/ip").args(arguments).output()?;
    ensure!(
        output.status.success() && output.stdout.len() <= 65536 && output.stderr.len() <= 4096,
        "shared pool network inspection failed or exceeded its bound"
    );
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn validate_network(addresses: &Value, routes: &Value) -> Result<()> {
    let interfaces = addresses.as_array().context("interface list absent")?;
    ensure!(
        interfaces.len() == 3,
        "shared server has unexpected interfaces"
    );
    for (name, address, prefix) in [
        ("lo", "127.0.0.1", 8),
        ("eth0", "10.0.2.100", 24),
        ("worker0", "192.0.2.1", 29),
    ] {
        let matches: Vec<_> = interfaces
            .iter()
            .filter(|entry| entry["ifname"] == name)
            .collect();
        ensure!(
            matches.len() == 1,
            "shared server interface absent or duplicated"
        );
        let interface = matches[0];
        ensure!(
            interface["flags"]
                .as_array()
                .context("interface flags absent")?
                .iter()
                .any(|flag| flag == "UP"),
            "shared server interface is down"
        );
        let ipv4: Vec<_> = interface["addr_info"]
            .as_array()
            .context("interface addresses absent")?
            .iter()
            .filter(|entry| entry["family"] == "inet")
            .collect();
        ensure!(
            ipv4.len() == 1 && ipv4[0]["local"] == address && ipv4[0]["prefixlen"] == prefix,
            "shared server IPv4 address differs"
        );
    }
    let routes = routes.as_array().context("route list absent")?;
    ensure!(routes.len() == 2, "shared server routing table differs");
    for (destination, interface) in [("10.0.2.0/24", "eth0"), ("192.0.2.0/29", "worker0")] {
        let selected: Vec<_> = routes
            .iter()
            .filter(|entry| entry["dst"] == destination)
            .collect();
        ensure!(
            selected.len() == 1
                && selected[0]["dev"] == interface
                && selected[0]["gateway"].is_null()
                && selected[0]["scope"] == "link",
            "shared server route is not its directly connected private network"
        );
    }
    Ok(())
}

pub fn require() -> Result<()> {
    ensure!(
        unsafe { libc::geteuid() } == 0,
        "shared pool startup requires its guest supervisor"
    );
    let (manifest, _) = shared_pool::read(Path::new(shared_pool::GUEST_MANIFEST))?;
    ensure!(
        hash_file(&std::env::current_exe()?)? == manifest.pool_verifier,
        "shared pool verifier differs from selected manifest"
    );
    ensure!(
        fs::read_to_string("/proc/sys/kernel/osrelease")?.trim() == "7.2.9-arch1-1",
        "shared pool kernel release differs"
    );
    let ipv6_present = Path::new("/proc/net/if_inet6").exists();
    for path in [
        "/proc/sys/net/ipv4/ip_forward",
        "/proc/sys/net/ipv4/conf/all/forwarding",
        "/proc/sys/net/ipv4/conf/eth0/forwarding",
        "/proc/sys/net/ipv4/conf/worker0/forwarding",
        "/proc/sys/net/ipv6/conf/all/forwarding",
        "/proc/sys/net/ipv6/conf/eth0/forwarding",
        "/proc/sys/net/ipv6/conf/worker0/forwarding",
    ] {
        if path.contains("ipv6") && !ipv6_present {
            ensure!(
                !Path::new(path).exists(),
                "inconsistent IPv6 subsystem state"
            );
            continue;
        }
        ensure!(
            fs::read_to_string(path)?.trim() == "0",
            "shared server forwards network traffic"
        );
    }
    let addresses = inspect(&["-j", "address", "show"])?;
    let routes = inspect(&["-j", "-4", "route", "show"])?;
    let ipv6_routes = if ipv6_present {
        inspect(&["-j", "-6", "route", "show", "default"])?
    } else {
        json!([])
    };
    ensure!(
        ipv6_routes.as_array().is_some_and(Vec::is_empty),
        "shared server has an IPv6 default route"
    );
    validate_network(&addresses, &routes)?;
    println!(
        "MD_SIM_SHARED_POOL_NETWORK {}",
        json!({
            "manifest":hash_file(Path::new(shared_pool::GUEST_MANIFEST))?,
            "verifier":manifest.pool_verifier,"input":hash_file(Path::new("/private-pool.json"))?,
            "addresses":addresses,"routes":routes,"ipv6_default_routes":ipv6_routes,
        "forwarding":false,"ipv6_subsystem_present":ipv6_present,
            "scope":"Independent pool in the SSH server network namespace. Outer evidence must prove the NIC endpoint, support image and kernel identities."
        })
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, os::unix::fs::PermissionsExt};

    #[test]
    fn cancellation_requires_the_exact_private_owned_file() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("cancel");
        let owner = unsafe { libc::geteuid() };
        assert!(!read_cancel(&path, owner)?);
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)?;
        file.write_all(b"cancel\n")?;
        file.sync_all()?;
        assert!(read_cancel(&path, owner)?);
        assert!(read_cancel(&path, owner.wrapping_add(1)).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640))?;
        assert!(read_cancel(&path, owner).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        fs::write(&path, b"cancel!")?;
        assert!(read_cancel(&path, owner).is_err());
        fs::write(&path, b"cancel\nextra")?;
        assert!(read_cancel(&path, owner).is_err());
        fs::write(&path, b"cancel\n")?;
        let linked = directory.path().join("linked");
        fs::hard_link(&path, &linked)?;
        assert!(read_cancel(&path, owner).is_err());
        fs::remove_file(&linked)?;
        assert!(read_cancel(&path, owner)?);
        std::os::unix::fs::symlink(&path, &linked)?;
        assert!(read_cancel(&linked, owner).is_err());
        Ok(())
    }

    #[test]
    fn cancellation_fifo_without_a_writer_is_rejected() -> Result<()> {
        use std::os::unix::ffi::OsStrExt;
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("cancel");
        let name = std::ffi::CString::new(path.as_os_str().as_bytes())?;
        ensure!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) } == 0);
        assert!(read_cancel(&path, unsafe { libc::geteuid() }).is_err());
        Ok(())
    }

    fn network() -> (Value, Value) {
        (
            json!([
                {"ifname":"lo","flags":["LOOPBACK","UP"],"addr_info":[{"family":"inet","local":"127.0.0.1","prefixlen":8}]},
                {"ifname":"eth0","flags":["BROADCAST","UP"],"addr_info":[{"family":"inet","local":"10.0.2.100","prefixlen":24}]},
                {"ifname":"worker0","flags":["BROADCAST","UP"],"addr_info":[{"family":"inet","local":"192.0.2.1","prefixlen":29}]}
            ]),
            json!([
                {"dst":"10.0.2.0/24","dev":"eth0","scope":"link"},
                {"dst":"192.0.2.0/29","dev":"worker0","scope":"link"}
            ]),
        )
    }

    #[test]
    fn only_the_declared_two_private_networks_are_admitted() {
        let (addresses, routes) = network();
        validate_network(&addresses, &routes).unwrap();
        let mut wrong = addresses.clone();
        wrong[1]["addr_info"][0]["local"] = json!("10.0.2.15");
        assert!(validate_network(&wrong, &routes).is_err());
        let mut wrong = addresses.clone();
        wrong[2]["ifname"] = json!("eth0");
        assert!(validate_network(&wrong, &routes).is_err());
        let mut wrong = addresses.clone();
        wrong
            .as_array_mut()
            .unwrap()
            .push(json!({"ifname":"host0"}));
        assert!(validate_network(&wrong, &routes).is_err());
        let mut wrong = routes.clone();
        wrong[0]["gateway"] = json!("10.0.2.1");
        assert!(validate_network(&addresses, &wrong).is_err());
        let mut wrong = routes;
        wrong
            .as_array_mut()
            .unwrap()
            .push(json!({"dst":"default","gateway":"192.0.2.6","dev":"worker0"}));
        assert!(validate_network(&addresses, &wrong).is_err());
    }
}
