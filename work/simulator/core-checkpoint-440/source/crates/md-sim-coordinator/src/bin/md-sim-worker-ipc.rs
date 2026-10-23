use anyhow::{ensure, Result};
use std::{
    fs::File,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    os::unix::net::UnixStream,
    path::Path,
};

#[path = "md-sim-worker-ipc/carrier.rs"]
mod carrier;

fn take_socket(fd: i32) -> Result<OwnedFd> {
    ensure!(fd > 2, "worker descriptor must not replace standard I/O");
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    ensure!(
        unsafe { libc::fstat(fd, stat.as_mut_ptr()) } == 0,
        "inherited descriptor absent"
    );
    ensure!(
        unsafe { stat.assume_init() }.st_mode & libc::S_IFMT == libc::S_IFSOCK,
        "inherited channel must be a socket"
    );
    // Only the trusted worker owns this descriptor. Its subprocesses must not
    // inherit worker control or support-VM endpoints.
    ensure!(
        unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } == 0,
        "cannot protect inherited worker descriptor"
    );
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    ensure!(
        args.len() == 3 || (args.len() == 6 && args[3] == "--private-carrier"),
        "usage: md-sim-worker-ipc WORKER_ROOT INHERITED_CONNECTED_FD [--private-carrier CONFIG_JSON SUPPORT_FD]"
    );
    let fd: i32 = args[2].parse()?;
    // The trusted parent transfers ownership of this private connected channel.
    let channel = UnixStream::from(take_socket(fd)?);
    let provider = if args.len() == 6 {
        let peer_fd: i32 = args[5].parse()?;
        ensure!(
            peer_fd != fd,
            "worker control and support descriptors must differ"
        );
        let peer = File::from(take_socket(peer_fd)?);
        use std::os::unix::fs::MetadataExt;
        let control_stat = File::from(OwnedFd::from(channel.try_clone()?)).metadata()?;
        let peer_stat = peer.metadata()?;
        ensure!(
            (control_stat.dev(), control_stat.ino()) != (peer_stat.dev(), peer_stat.ino()),
            "support descriptor aliases the worker control channel"
        );
        Some(carrier::Provider::start(
            Path::new(&args[4]),
            peer,
            Path::new(&args[1]),
        )?)
    } else {
        None
    };
    let incoming = std::iter::from_fn(|| {
        let mut event = libc::pollfd {
            fd: channel.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        loop {
            let status = unsafe { libc::poll(&mut event, 1, 1000) };
            if status < 0 {
                let e = std::io::Error::last_os_error();
                if e.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Some(Err(e.into()));
            }
            if event.revents & (libc::POLLERR | libc::POLLNVAL | libc::POLLHUP) != 0 {
                return None;
            }
            if event.revents & libc::POLLIN != 0 {
                return Some(channel.try_clone().map_err(Into::into));
            }
        }
    });
    if let Some((provider, binding)) = provider {
        let result = md_sim_runtime::worker::serve_connections_with_channel_owned(
            provider.root(),
            incoming,
            Default::default(),
            binding,
        );
        let retained = provider.finish();
        result?;
        retained
    } else {
        md_sim_runtime::worker::serve_connections(Path::new(&args[1]), incoming)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::fd::IntoRawFd;

    #[test]
    fn inherited_control_descriptor_is_cloexec_and_files_are_refused() {
        let (channel, _peer) = UnixStream::pair().unwrap();
        let fd = channel.into_raw_fd();
        assert_eq!(unsafe { libc::fcntl(fd, libc::F_SETFD, 0) }, 0);
        let owned = take_socket(fd).unwrap();
        assert_ne!(
            unsafe { libc::fcntl(owned.as_raw_fd(), libc::F_GETFD) } & libc::FD_CLOEXEC,
            0
        );
        let file = File::open("/dev/null").unwrap();
        assert!(take_socket(file.as_raw_fd()).is_err());
        assert!(take_socket(0).is_err());
    }
}
