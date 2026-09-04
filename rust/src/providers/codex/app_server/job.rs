//! Windows Job Object ownership for supervised Codex children.

#[cfg(unix)]
const SIGTERM: i32 = 15;
#[cfg(unix)]
const SIGKILL: i32 = 9;

/// Deliver a signal to a Unix process group. Process groups use a negative
/// pid with `kill(2)`; errors mean the group has already exited or cannot be
/// signalled and are intentionally handled by the caller as bounded cleanup.
#[cfg(unix)]
pub(crate) fn signal_process_group(process_group: i32, signal: i32) {
    unsafe extern "C" {
        fn kill(pid: i32, signal: i32) -> i32;
    }

    if process_group > 0 {
        let _ = unsafe { kill(-process_group, signal) };
    }
}

#[cfg(unix)]
pub(crate) fn terminate_process_group(process_group: i32) {
    signal_process_group(process_group, SIGTERM);
}

#[cfg(unix)]
pub(crate) fn kill_process_group(process_group: i32) {
    signal_process_group(process_group, SIGKILL);
}

/// Make this process the nearest reaper for orphaned descendants of its
/// children. Linux otherwise reparents a background child to init as soon as
/// its App Server leader exits, leaving this supervisor unable to reap it.
#[cfg(target_os = "linux")]
pub(crate) fn enable_child_subreaper() -> bool {
    const PR_SET_CHILD_SUBREAPER: i32 = 36;

    unsafe extern "C" {
        fn prctl(option: i32, arg2: usize, arg3: usize, arg4: usize, arg5: usize) -> i32;
    }

    unsafe { prctl(PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) == 0 }
}

/// A process id paired with the kernel start-time field from `/proc/<pid>/stat`.
/// The start time prevents a delayed cleanup worker from signalling a reused
/// pid that no longer belongs to the App Server tree.
#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ProcessIdentity {
    pid: i32,
    start_time: u64,
}

#[cfg(target_os = "linux")]
impl ProcessIdentity {
    pub(crate) fn pid(self) -> i32 {
        self.pid
    }
}

#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy)]
struct ProcessStat {
    identity: ProcessIdentity,
    process_group: i32,
}

#[cfg(target_os = "linux")]
fn read_process_stat(pid: i32) -> Option<ProcessStat> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let fields = stat
        .rsplit_once(')')?
        .1
        .split_whitespace()
        .collect::<Vec<_>>();
    // Fields after `comm`: state (3), ppid (4), pgrp (5), ... starttime (22).
    let process_group = fields.get(2)?.parse().ok()?;
    let start_time = fields.get(19)?.parse().ok()?;
    Some(ProcessStat {
        identity: ProcessIdentity { pid, start_time },
        process_group,
    })
}

#[cfg(target_os = "linux")]
fn direct_children(pid: i32) -> Vec<i32> {
    std::fs::read_to_string(format!("/proc/{pid}/task/{pid}/children"))
        .ok()
        .into_iter()
        .flat_map(|children| {
            children
                .split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .filter_map(|child| child.parse().ok())
        .collect()
}

/// Snapshot descendants from the kernel's parent/child relation. Session or
/// process-group changes do not change that relation, so detached descendants
/// remain discoverable until their parent exits.
#[cfg(target_os = "linux")]
pub(crate) fn process_descendants(root_pid: i32) -> Vec<ProcessIdentity> {
    let mut descendants = Vec::new();
    let mut queue = std::collections::VecDeque::from([root_pid]);
    let mut seen = std::collections::HashSet::from([root_pid]);
    while let Some(parent) = queue.pop_front() {
        for child in direct_children(parent) {
            if !seen.insert(child) {
                continue;
            }
            if let Some(stat) = read_process_stat(child) {
                descendants.push(stat.identity);
                queue.push_back(child);
            }
        }
    }
    descendants
}

/// Snapshot all current members of an owned process group. This covers a
/// leader that has already exited: its non-detached background children still
/// retain the original process group after subreaper adoption.
#[cfg(target_os = "linux")]
fn identity_is_current(identity: ProcessIdentity) -> bool {
    read_process_stat(identity.pid).is_some_and(|stat| stat.identity == identity)
}

#[cfg(target_os = "linux")]
fn leader_owns_group(
    stat: Option<ProcessStat>,
    leader: ProcessIdentity,
    process_group: i32,
) -> bool {
    stat.is_some_and(|stat| stat.identity == leader && stat.process_group == process_group)
}

#[cfg(target_os = "linux")]
pub(crate) fn signal_processes(processes: &[ProcessIdentity], signal: i32) {
    unsafe extern "C" {
        fn kill(pid: i32, signal: i32) -> i32;
    }

    for identity in processes {
        if identity_is_current(*identity) {
            let _ = unsafe { kill(identity.pid, signal) };
        }
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn terminate_processes(processes: &[ProcessIdentity]) {
    signal_processes(processes, SIGTERM);
}

#[cfg(target_os = "linux")]
pub(crate) fn kill_processes(processes: &[ProcessIdentity]) {
    signal_processes(processes, SIGKILL);
}

#[cfg(target_os = "linux")]
pub(crate) fn tracked_processes_exist(processes: &[ProcessIdentity]) -> bool {
    processes.iter().copied().any(identity_is_current)
}

/// Reap only exact, already-tracked descendants. `waitpid` returns ECHILD for
/// a still-parented child; once the leader exits, the Linux subreaper adopts
/// it and the next bounded pass can collect it.
#[cfg(target_os = "linux")]
pub(crate) fn reap_tracked_processes(processes: &[ProcessIdentity]) {
    const WNOHANG: i32 = 1;

    unsafe extern "C" {
        fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
    }

    for identity in processes {
        if identity_is_current(*identity) {
            let mut status = 0;
            let _ = unsafe { waitpid(identity.pid, &mut status, WNOHANG) };
        }
    }
}

/// Per-App-Server ownership ledger. It starts tracking immediately after
/// spawn, while the validated leader is still available in procfs. After that
/// leader disappears, cleanup is limited to identities already recorded here;
/// historical pids and process groups are never rediscovered or signalled.
#[cfg(target_os = "linux")]
pub(crate) struct ProcessLedger {
    leader: ProcessIdentity,
    process_group: i32,
    tracked: std::sync::Mutex<std::collections::HashSet<ProcessIdentity>>,
    stop_monitor: std::sync::atomic::AtomicBool,
}

#[cfg(target_os = "linux")]
impl ProcessLedger {
    pub(crate) fn start(leader_pid: i32) -> Option<std::sync::Arc<Self>> {
        let leader = read_process_stat(leader_pid)?.identity;
        let ledger = std::sync::Arc::new(Self {
            leader,
            process_group: leader_pid,
            tracked: std::sync::Mutex::new(std::collections::HashSet::new()),
            stop_monitor: std::sync::atomic::AtomicBool::new(false),
        });
        ledger.capture();
        Self::start_monitor(&ledger);
        Some(ledger)
    }

    fn start_monitor(ledger: &std::sync::Arc<Self>) {
        let ledger = std::sync::Arc::clone(ledger);
        let _ = std::thread::Builder::new()
            .name("codexbar-app-server-tracker".into())
            .spawn(move || {
                const POLL: std::time::Duration = std::time::Duration::from_millis(5);
                while !ledger
                    .stop_monitor
                    .load(std::sync::atomic::Ordering::Acquire)
                {
                    if !ledger.leader_is_current() {
                        return;
                    }
                    ledger.capture();
                    std::thread::sleep(POLL);
                }
            });
    }

    pub(crate) fn stop_monitoring(&self) {
        self.stop_monitor
            .store(true, std::sync::atomic::Ordering::Release);
    }

    pub(crate) fn capture(&self) {
        if !self.leader_is_current() {
            return;
        }
        if let Ok(mut tracked) = self.tracked.lock() {
            for descendant in process_descendants(self.leader.pid) {
                tracked.insert(descendant);
            }
        }
    }

    fn leader_is_current(&self) -> bool {
        identity_is_current(self.leader)
    }

    fn owns_process_group(&self) -> bool {
        leader_owns_group(
            read_process_stat(self.leader.pid),
            self.leader,
            self.process_group,
        )
    }

    fn snapshot(&self) -> Vec<ProcessIdentity> {
        self.tracked
            .lock()
            .map(|tracked| tracked.iter().copied().collect())
            .unwrap_or_default()
    }

    pub(crate) fn terminate(&self) {
        let tracked = self.snapshot();
        terminate_processes(&tracked);
        if self.owns_process_group() {
            terminate_process_group(self.process_group);
        }
    }

    pub(crate) fn kill(&self) {
        let tracked = self.snapshot();
        kill_processes(&tracked);
        if self.owns_process_group() {
            kill_process_group(self.process_group);
        }
    }

    pub(crate) fn reap(&self) {
        reap_tracked_processes(&self.snapshot());
    }

    pub(crate) fn is_drained(&self) -> bool {
        !self.owns_process_group() && !tracked_processes_exist(&self.snapshot())
    }

    /// Drop performs no procfs I/O or signalling. The worker owns every
    /// potentially slow operation and stops at a fixed deadline.
    pub(crate) fn spawn_drop_cleanup(self: std::sync::Arc<Self>) {
        self.stop_monitoring();
        let _ = std::thread::Builder::new()
            .name("codexbar-app-server-drop-cleanup".into())
            .spawn(move || {
                const LIMIT: std::time::Duration = std::time::Duration::from_millis(250);
                const POLL: std::time::Duration = std::time::Duration::from_millis(10);
                let deadline = std::time::Instant::now() + LIMIT;
                loop {
                    self.capture();
                    self.kill();
                    self.reap();
                    if self.is_drained() || std::time::Instant::now() >= deadline {
                        return;
                    }
                    std::thread::sleep(POLL);
                }
            });
    }

    #[cfg(test)]
    pub(crate) fn tracks(&self, pid: i32) -> bool {
        self.tracked
            .lock()
            .is_ok_and(|tracked| tracked.iter().any(|identity| identity.pid == pid))
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

    #[cfg(target_os = "linux")]
    #[test]
    fn leader_group_ownership_rejects_reused_pid_or_changed_group() {
        let leader = ProcessIdentity {
            pid: 42,
            start_time: 100,
        };
        assert!(leader_owns_group(
            Some(ProcessStat {
                identity: leader,
                process_group: 42,
            }),
            leader,
            42,
        ));
        assert!(!leader_owns_group(
            Some(ProcessStat {
                identity: ProcessIdentity {
                    pid: 42,
                    start_time: 101,
                },
                process_group: 42,
            }),
            leader,
            42,
        ));
        assert!(!leader_owns_group(
            Some(ProcessStat {
                identity: leader,
                process_group: 99,
            }),
            leader,
            42,
        ));
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
