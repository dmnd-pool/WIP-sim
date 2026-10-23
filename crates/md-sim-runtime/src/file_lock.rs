//! Exclusive file leases with an explicit owner-process release.
//! `flock` follows the open file description across fork. Closing only the
//! parent's descriptor can leave a lease held by an unrelated pre-exec child.
use std::{fs::File, io, os::fd::AsRawFd};

pub struct ExclusiveFileLock {
    file: File,
    owner: libc::pid_t,
}

impl ExclusiveFileLock {
    /// The caller must validate the path, inode and file contents first.
    /// Supply a newly opened file, not a duplicate of a leased descriptor.
    pub fn acquire(file: File) -> io::Result<Self> {
        // SAFETY: File owns a live descriptor throughout the call.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            file,
            // SAFETY: getpid has no preconditions.
            owner: unsafe { libc::getpid() },
        })
    }
}

impl Drop for ExclusiveFileLock {
    fn drop(&mut self) {
        // A fork child must not release the parent's lease. In the owner,
        // explicit unlock also releases inherited copies of this description.
        // SAFETY: File still owns the descriptor. Neither call allocates.
        unsafe {
            if libc::getpid() == self.owner {
                libc::flock(self.file.as_raw_fd(), libc::LOCK_UN);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::fd::{FromRawFd, OwnedFd};

    struct Child {
        pid: libc::pid_t,
        release: Option<OwnedFd>,
    }
    impl Drop for Child {
        fn drop(&mut self) {
            self.release.take();
            // SAFETY: owned child blocks on release pipe; closing it sends EOF.
            unsafe {
                while libc::waitpid(self.pid, std::ptr::null_mut(), 0) < 0
                    && io::Error::last_os_error().kind() == io::ErrorKind::Interrupted
                {
                }
            }
        }
    }

    fn pipe() -> [OwnedFd; 2] {
        let mut fds = [-1; 2];
        // SAFETY: writable array has exactly two descriptor slots.
        assert_eq!(unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) }, 0);
        // SAFETY: successful pipe2 created these unique owned descriptors.
        unsafe { [OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])] }
    }

    #[test]
    fn fork_child_cannot_release_parent_and_cannot_extend_released_lease() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("lease");
        let file = File::create(&path).unwrap();
        let inherited_copy = file.try_clone().unwrap();
        let lock = ExclusiveFileLock::acquire(file).unwrap();
        let [ready_read, ready_write] = pipe();
        let [release_read, release_write] = pipe();
        // SAFETY: the child calls only async-signal-safe getpid, close, read,
        // write and _exit. It does not allocate, unwind or invoke test code.
        let pid = unsafe { libc::fork() };
        assert!(pid >= 0);
        if pid == 0 {
            drop(ready_read);
            drop(release_write);
            drop(lock);
            unsafe {
                let byte = 1u8;
                if libc::write(ready_write.as_raw_fd(), (&byte as *const u8).cast(), 1) != 1 {
                    libc::_exit(2);
                }
                let mut byte = 0u8;
                libc::read(release_read.as_raw_fd(), (&mut byte as *mut u8).cast(), 1);
                libc::_exit(0);
            }
        }
        let _child = Child {
            pid,
            release: Some(release_write),
        };
        drop(ready_write);
        drop(release_read);
        let mut byte = 0u8;
        assert_eq!(
            unsafe { libc::read(ready_read.as_raw_fd(), (&mut byte as *mut u8).cast(), 1) },
            1
        );
        // The child's destructor must leave the parent's lock held.
        let error = ExclusiveFileLock::acquire(File::open(&path).unwrap())
            .err()
            .expect("parent still owns lease");
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        drop(lock);
        // Both processes still have inherited copies. Explicit release must
        // allow a new owner before either copy is closed or the child exits.
        let next = ExclusiveFileLock::acquire(File::open(&path).unwrap()).unwrap();
        drop(inherited_copy);
        assert!(ExclusiveFileLock::acquire(File::open(&path).unwrap()).is_err());
        drop(next);
        assert!(ExclusiveFileLock::acquire(File::open(&path).unwrap()).is_ok());
    }
}
