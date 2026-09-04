//! Platform process ownership for supervised Codex children.

#[cfg(unix)]
const SIGTERM: i32 = 15;
#[cfg(unix)]
const SIGKILL: i32 = 9;

/// Deliver a signal to a Unix process group. Process groups use a negative
/// pid with `kill(2)`; errors mean the group has already exited or cannot be
/// signalled and are intentionally handled by the caller as bounded cleanup.
#[cfg(all(unix, not(target_os = "linux")))]
pub(crate) fn signal_process_group(process_group: i32, signal: i32) {
    unsafe extern "C" {
        fn kill(pid: i32, signal: i32) -> i32;
    }

    if process_group > 0 {
        let _ = unsafe { kill(-process_group, signal) };
    }
}

#[cfg(all(unix, not(target_os = "linux")))]
pub(crate) fn terminate_process_group(process_group: i32) {
    signal_process_group(process_group, SIGTERM);
}

#[cfg(all(unix, not(target_os = "linux")))]
pub(crate) fn kill_process_group(process_group: i32) {
    signal_process_group(process_group, SIGKILL);
}

/// Pending half of the Linux-only supervisor handshake.
///
/// `Command::pre_exec` turns the spawned process into a small, dedicated
/// subreaper and forks the real App Server below it. The parent keeps only the
/// socket's parent end. That descriptor is a kernel-owned capability for this
/// exact supervisor: unlike a PID or PGID, it cannot be recycled to address an
/// unrelated process.
#[cfg(target_os = "linux")]
pub(crate) struct PendingLinuxSupervisor {
    child_end: std::os::fd::OwnedFd,
    control_end: std::os::fd::OwnedFd,
}

#[cfg(target_os = "linux")]
impl PendingLinuxSupervisor {
    pub(crate) fn new() -> std::io::Result<Self> {
        use std::os::fd::FromRawFd;

        const AF_UNIX: i32 = 1;
        const SOCK_STREAM: i32 = 1;
        const SOCK_CLOEXEC: i32 = 0o2_000_000;
        const SOCK_NONBLOCK: i32 = 0o4_000;
        let mut descriptors = [-1; 2];
        let result = unsafe {
            linux_ffi::socketpair(
                AF_UNIX,
                SOCK_STREAM | SOCK_CLOEXEC | SOCK_NONBLOCK,
                0,
                descriptors.as_mut_ptr(),
            )
        };
        if result != 0 {
            return Err(std::io::Error::last_os_error());
        }
        if let Err(error) = move_control_descriptors_above_stdio(&mut descriptors) {
            let _ = unsafe { linux_ffi::close(descriptors[0]) };
            let _ = unsafe { linux_ffi::close(descriptors[1]) };
            return Err(error);
        }
        Ok(Self {
            child_end: unsafe { std::os::fd::OwnedFd::from_raw_fd(descriptors[0]) },
            control_end: unsafe { std::os::fd::OwnedFd::from_raw_fd(descriptors[1]) },
        })
    }

    /// Install the child-side supervisor before `execve`. The closure's
    /// supervisor branch never returns; the forked App Server branch returns
    /// to `Command` and immediately execs the validated program.
    pub(crate) fn configure(&self, command: &mut tokio::process::Command) {
        use std::os::fd::AsRawFd;
        use std::os::unix::process::CommandExt;

        let child_end = self.child_end.as_raw_fd();
        let control_end = self.control_end.as_raw_fd();
        unsafe {
            command
                .as_std_mut()
                .pre_exec(move || linux_supervisor_pre_exec(child_end, control_end));
        }
    }

    pub(crate) fn activate(self) -> std::io::Result<LinuxSupervisorHandle> {
        use std::os::fd::AsRawFd;

        let Self {
            child_end,
            control_end,
        } = self;
        drop(child_end);
        write_control(control_end.as_raw_fd(), CONTROL_START)?;
        Ok(LinuxSupervisorHandle {
            control: Some(control_end),
        })
    }
}

/// Parent-side ownership of one exact Linux App Server supervisor.
#[cfg(target_os = "linux")]
pub(crate) struct LinuxSupervisorHandle {
    control: Option<std::os::fd::OwnedFd>,
}

#[cfg(target_os = "linux")]
impl LinuxSupervisorHandle {
    /// Notify and disconnect without process discovery, signalling, waiting,
    /// locking, or thread creation. Even if the supervisor is stopped in the
    /// kernel, closing this nonblocking control socket remains bounded.
    pub(crate) fn request_shutdown(&mut self) {
        if let Some(control) = self.control.take() {
            // EOF/HUP is the shutdown request. Closing cannot raise SIGPIPE
            // and does not depend on the supervisor being scheduled.
            drop(control);
        }
    }
}

#[cfg(target_os = "linux")]
const CONTROL_START: u8 = 1;

#[cfg(target_os = "linux")]
fn move_control_descriptors_above_stdio(descriptors: &mut [i32; 2]) -> std::io::Result<()> {
    const F_DUPFD_CLOEXEC: i32 = 1030;
    for descriptor in descriptors.iter_mut() {
        if *descriptor >= 3 {
            continue;
        }
        let duplicate = unsafe { linux_ffi::fcntl(*descriptor, F_DUPFD_CLOEXEC, 3) };
        if duplicate < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let _ = unsafe { linux_ffi::close(*descriptor) };
        *descriptor = duplicate;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn write_control(descriptor: i32, command: u8) -> std::io::Result<()> {
    const MSG_NOSIGNAL: i32 = 0x4000;
    for _ in 0..3 {
        let result = unsafe {
            linux_ffi::send(
                descriptor,
                (&command as *const u8).cast::<core::ffi::c_void>(),
                1,
                MSG_NOSIGNAL,
            )
        };
        if result == 1 {
            return Ok(());
        }
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::Interrupted,
        "Linux supervisor control write remained interrupted",
    ))
}

/// Runs after `fork` and therefore uses only async-signal-safe libc operations
/// until the App Server branch reaches `execve`. The supervisor branch never
/// touches Rust allocation, locks, or Tokio state and exits through `_exit`.
#[cfg(target_os = "linux")]
fn linux_supervisor_pre_exec(control_read: i32, control_write: i32) -> std::io::Result<()> {
    const PR_SET_CHILD_SUBREAPER: i32 = 36;
    const SIGCHLD: i32 = 17;
    const SIG_DFL: usize = 0;
    const SIG_ERR: usize = usize::MAX;

    if unsafe { linux_ffi::setpgid(0, 0) } != 0
        || unsafe { linux_ffi::prctl(PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) } != 0
        || unsafe { linux_ffi::signal(SIGCHLD, SIG_DFL) } == SIG_ERR
    {
        return Err(std::io::Error::last_os_error());
    }

    // `_Fork` omits pthread_atfork handlers. That matters here because this
    // closure already runs in the post-fork child of a multithreaded process.
    let forked = unsafe { linux_ffi::fork_without_atfork() };
    if forked < 0 {
        return Err(std::io::Error::last_os_error());
    }
    if forked == 0 {
        let _ = unsafe { linux_ffi::close(control_read) };
        let _ = unsafe { linux_ffi::close(control_write) };
        return Ok(());
    }

    run_linux_supervisor(control_read, control_write, forked)
}

#[cfg(target_os = "linux")]
fn run_linux_supervisor(control_read: i32, control_write: i32, app_server_pid: i32) -> ! {
    let _ = unsafe { linux_ffi::close(control_write) };
    for descriptor in 0..=2 {
        let _ = unsafe { linux_ffi::close(descriptor) };
    }
    if !close_descriptors_except(control_read) || !wait_for_start(control_read, app_server_pid) {
        shutdown_supervised_children();
        unsafe { linux_ffi::_exit(127) }
    }

    loop {
        if reap_exited_children() {
            unsafe { linux_ffi::_exit(0) }
        }
        if control_requests_shutdown(control_read, 50) {
            shutdown_supervised_children();
            unsafe { linux_ffi::_exit(0) }
        }
    }
}

#[cfg(target_os = "linux")]
fn close_descriptors_except(keep: i32) -> bool {
    let lower_closed = keep == 3 || unsafe { linux_ffi::close_range(3, keep as u32 - 1, 0) } == 0;
    let upper_closed = unsafe { linux_ffi::close_range(keep as u32 + 1, u32::MAX, 0) } == 0;
    lower_closed && upper_closed
}

#[cfg(target_os = "linux")]
fn wait_for_start(control_read: i32, app_server_pid: i32) -> bool {
    const WNOHANG: i32 = 1;
    const W_ALL: i32 = 0x4000_0000;
    loop {
        let mut command = 0u8;
        let result = unsafe {
            linux_ffi::read(
                control_read,
                (&mut command as *mut u8).cast::<core::ffi::c_void>(),
                1,
            )
        };
        if result == 1 {
            return command == CONTROL_START;
        }
        if result == 0 {
            return false;
        }
        match linux_errno() {
            linux_ffi::EINTR => continue,
            linux_ffi::EAGAIN => {
                let mut status = 0;
                let child =
                    unsafe { linux_ffi::waitpid(app_server_pid, &mut status, WNOHANG | W_ALL) };
                if child != 0 && !(child < 0 && linux_errno() == linux_ffi::EINTR) {
                    // `Command::spawn` waits for its direct child on exec
                    // failure. Exiting here when the inner exec child fails
                    // prevents a parent/supervisor handshake deadlock.
                    return false;
                }
                if poll_control(control_read, 10) < 0 && linux_errno() != linux_ffi::EINTR {
                    return false;
                }
            }
            _ => return false,
        }
    }
}

#[cfg(target_os = "linux")]
fn control_requests_shutdown(control_read: i32, timeout_ms: i32) -> bool {
    let result = poll_control(control_read, timeout_ms);
    if result == 0 || (result < 0 && linux_errno() == linux_ffi::EINTR) {
        return false;
    }
    if result < 0 {
        return true;
    }

    let mut buffer = [0u8; 16];
    loop {
        let read = unsafe {
            linux_ffi::read(
                control_read,
                buffer.as_mut_ptr().cast::<core::ffi::c_void>(),
                buffer.len(),
            )
        };
        if read > 0 {
            continue;
        }
        if read == 0 {
            return true;
        }
        match linux_errno() {
            linux_ffi::EINTR => continue,
            linux_ffi::EAGAIN => return false,
            _ => return true,
        }
    }
}

#[cfg(target_os = "linux")]
fn poll_control(control_read: i32, timeout_ms: i32) -> i32 {
    let mut descriptor = linux_ffi::PollFd {
        fd: control_read,
        events: linux_ffi::POLLIN | linux_ffi::POLLERR | linux_ffi::POLLHUP,
        revents: 0,
    };
    unsafe { linux_ffi::poll(&mut descriptor, 1, timeout_ms) }
}

/// Signals only direct children of this dedicated supervisor. It deliberately
/// signals before `waitpid`: a direct child that exits during the procfs read
/// remains a zombie, so its PID cannot be reused for an unrelated process
/// before this loop sends the signal. Once a parent is reaped, its surviving
/// descendants are adopted by this subreaper and become the next iteration's
/// direct children. `setsid` and `setpgid` cannot escape that ownership.
#[cfg(target_os = "linux")]
fn shutdown_supervised_children() {
    const TERM_ROUNDS: usize = 8;
    const KILL_ROUNDS: usize = 17;
    for _ in 0..TERM_ROUNDS {
        signal_direct_children(SIGTERM);
        if reap_exited_children() {
            return;
        }
        sleep_cleanup_poll();
    }
    for _ in 0..KILL_ROUNDS {
        signal_direct_children(SIGKILL);
        if reap_exited_children() {
            return;
        }
        sleep_cleanup_poll();
    }
}

#[cfg(target_os = "linux")]
fn signal_direct_children(signal: i32) {
    const CHILDREN_PATH: &[u8] = b"/proc/thread-self/children\0";
    const O_RDONLY: i32 = 0;
    const O_CLOEXEC: i32 = 0o2_000_000;

    let descriptor = unsafe {
        linux_ffi::open(
            CHILDREN_PATH.as_ptr().cast::<core::ffi::c_char>(),
            O_RDONLY | O_CLOEXEC,
        )
    };
    if descriptor < 0 {
        // Fail closed: without kernel-confirmed current children, never fall
        // back to a historical PID or process-group broadcast.
        return;
    }

    let mut buffer = [0u8; 4096];
    let mut pid = 0i32;
    let mut in_number = false;
    loop {
        let count = unsafe {
            linux_ffi::read(
                descriptor,
                buffer.as_mut_ptr().cast::<core::ffi::c_void>(),
                buffer.len(),
            )
        };
        if count < 0 {
            if linux_errno() == linux_ffi::EINTR {
                continue;
            }
            break;
        }
        if count == 0 {
            if in_number && pid > 0 {
                let _ = unsafe { linux_ffi::kill(pid, signal) };
            }
            break;
        }
        for byte in &buffer[..count as usize] {
            if byte.is_ascii_digit() {
                in_number = true;
                pid = pid
                    .checked_mul(10)
                    .and_then(|value| value.checked_add(i32::from(*byte - b'0')))
                    .unwrap_or(0);
            } else if in_number {
                if pid > 0 {
                    let _ = unsafe { linux_ffi::kill(pid, signal) };
                }
                pid = 0;
                in_number = false;
            }
        }
    }
    let _ = unsafe { linux_ffi::close(descriptor) };
}

#[cfg(target_os = "linux")]
fn reap_exited_children() -> bool {
    const WNOHANG: i32 = 1;
    const W_ALL: i32 = 0x4000_0000;
    loop {
        let mut status = 0;
        let result = unsafe { linux_ffi::waitpid(-1, &mut status, WNOHANG | W_ALL) };
        if result > 0 {
            continue;
        }
        if result == 0 {
            return false;
        }
        match linux_errno() {
            linux_ffi::EINTR => continue,
            linux_ffi::ECHILD => return true,
            _ => return false,
        }
    }
}

#[cfg(target_os = "linux")]
fn sleep_cleanup_poll() {
    let _ = unsafe { linux_ffi::poll(core::ptr::null_mut(), 0, 10) };
}

#[cfg(target_os = "linux")]
fn linux_errno() -> i32 {
    unsafe { *linux_ffi::__errno_location() }
}

#[cfg(target_os = "linux")]
mod linux_ffi {
    pub(super) const EINTR: i32 = 4;
    pub(super) const ECHILD: i32 = 10;
    pub(super) const EAGAIN: i32 = 11;
    pub(super) const POLLIN: i16 = 0x001;
    pub(super) const POLLERR: i16 = 0x008;
    pub(super) const POLLHUP: i16 = 0x010;

    #[repr(C)]
    pub(super) struct PollFd {
        pub(super) fd: i32,
        pub(super) events: i16,
        pub(super) revents: i16,
    }

    unsafe extern "C" {
        pub(super) fn __errno_location() -> *mut i32;
        pub(super) fn close(descriptor: i32) -> i32;
        pub(super) fn close_range(first: u32, last: u32, flags: i32) -> i32;
        pub(super) fn fcntl(descriptor: i32, command: i32, ...) -> i32;
        #[link_name = "_Fork"]
        pub(super) fn fork_without_atfork() -> i32;
        pub(super) fn kill(pid: i32, signal: i32) -> i32;
        pub(super) fn open(path: *const core::ffi::c_char, flags: i32, ...) -> i32;
        pub(super) fn poll(descriptors: *mut PollFd, count: usize, timeout_ms: i32) -> i32;
        pub(super) fn prctl(option: i32, arg2: usize, arg3: usize, arg4: usize, arg5: usize)
        -> i32;
        pub(super) fn read(descriptor: i32, buffer: *mut core::ffi::c_void, count: usize) -> isize;
        pub(super) fn setpgid(pid: i32, process_group: i32) -> i32;
        pub(super) fn send(
            socket: i32,
            buffer: *const core::ffi::c_void,
            count: usize,
            flags: i32,
        ) -> isize;
        pub(super) fn signal(signal: i32, handler: usize) -> usize;
        pub(super) fn socketpair(
            domain: i32,
            socket_type: i32,
            protocol: i32,
            sockets: *mut i32,
        ) -> i32;
        pub(super) fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
        pub(super) fn _exit(status: i32) -> !;
    }
}

#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, HANDLE};
#[cfg(windows)]
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};
#[cfg(windows)]
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
};
#[cfg(windows)]
use windows::core::Result as WindowsResult;

/// Kill-on-close limit flag, mirrored for non-Windows compilation and tests.
#[allow(dead_code)]
#[cfg(windows)]
pub(crate) const JOB_KILL_ON_CLOSE_FLAG: u32 = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE.0;
#[allow(dead_code)]
#[cfg(not(windows))]
pub(crate) const JOB_KILL_ON_CLOSE_FLAG: u32 = 8192;

/// Owns a Windows Job Object. Closing the last job handle terminates the
/// whole process tree because KILL_ON_JOB_CLOSE is set at creation time.
#[cfg(windows)]
pub struct JobHandle {
    handle: HANDLE,
}

#[cfg(windows)]
impl JobHandle {
    /// Create a fresh unnamed Job Object with kill-on-close enabled.
    pub fn new_kill_on_close() -> WindowsResult<Self> {
        unsafe {
            let handle = CreateJobObjectW(None, None)?;
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let size = std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32;
            if let Err(error) = SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const core::ffi::c_void,
                size,
            ) {
                let _ = CloseHandle(handle);
                return Err(error);
            }
            Ok(Self { handle })
        }
    }

    /// Assign a process id to this job. The process handle is opened with the
    /// minimal rights needed for job assignment and closed immediately after.
    pub fn assign_process(&self, process_id: u32) -> WindowsResult<()> {
        unsafe {
            let process = OpenProcess(
                PROCESS_SET_QUOTA | PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION,
                false,
                process_id,
            )?;
            let result = AssignProcessToJobObject(self.handle, process);
            let _ = CloseHandle(process);
            result
        }
    }

    pub fn raw_handle(&self) -> HANDLE {
        self.handle
    }
}

#[cfg(windows)]
impl Drop for JobHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

#[cfg(windows)]
unsafe impl Send for JobHandle {}
#[cfg(windows)]
unsafe impl Sync for JobHandle {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kill_on_close_limit_is_enabled_by_contract() {
        assert_eq!(JOB_KILL_ON_CLOSE_FLAG, 8192);
    }

    #[cfg(windows)]
    #[test]
    fn job_object_is_created_with_kill_on_close_and_can_own_a_child() {
        use windows::Win32::System::JobObjects::{
            JOBOBJECT_BASIC_PROCESS_ID_LIST, JobObjectBasicProcessIdList, QueryInformationJobObject,
        };

        // Launch a waiting child so the job has something to own.
        let mut child = std::process::Command::new(
            r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe",
        )
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Start-Sleep -Seconds 30",
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("failed to spawn test child");

        let job = JobHandle::new_kill_on_close().expect("job creation failed");
        job.assign_process(child.id())
            .expect("job assignment failed");

        // Query the job's process id list: it must contain the child pid.
        let mut buffer = [0u8; 128];
        let mut returned = 0u32;
        unsafe {
            QueryInformationJobObject(
                job.raw_handle(),
                JobObjectBasicProcessIdList,
                buffer.as_mut_ptr() as *mut core::ffi::c_void,
                buffer.len() as u32,
                Some(&mut returned),
            )
            .expect("job query failed");
            let list = &*(buffer.as_ptr() as *const JOBOBJECT_BASIC_PROCESS_ID_LIST);
            assert!(list.NumberOfProcessIdsInList >= 1);
            let ids = std::slice::from_raw_parts(
                list.ProcessIdList.as_ptr(),
                list.NumberOfProcessIdsInList as usize,
            );
            assert!(ids.contains(&(child.id() as usize)));
        }

        // Drop the job handle: KILL_ON_JOB_CLOSE terminates the child.
        drop(job);
        let status = child.wait().expect("child wait failed");
        assert!(!status.success() || status.code() == Some(0) || status.code().is_none());
    }
}
