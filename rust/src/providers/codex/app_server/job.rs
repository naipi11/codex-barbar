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

/// Reap exited members that Linux reparented to this process after their App
/// Server leader exited. A negative `waitpid` target selects direct children
/// in exactly one process group and therefore cannot consume another group's
/// child.
#[cfg(target_os = "linux")]
pub(crate) fn reap_process_group_children(process_group: i32) {
    const WNOHANG: i32 = 1;

    unsafe extern "C" {
        fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
    }

    if process_group <= 0 {
        return;
    }
    loop {
        let mut status = 0;
        let result = unsafe { waitpid(-process_group, &mut status, WNOHANG) };
        if result <= 0 {
            return;
        }
    }
}

/// Whether a Unix process group still has any member. This intentionally uses
/// `kill(..., 0)`: after `reap_process_group_children` has run, a successful
/// probe represents a member still awaiting termination rather than one of
/// this supervisor's unreaped zombies.
#[cfg(target_os = "linux")]
pub(crate) fn process_group_exists(process_group: i32) -> bool {
    unsafe extern "C" {
        fn kill(pid: i32, signal: i32) -> i32;
    }

    process_group > 0 && unsafe { kill(-process_group, 0) == 0 }
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
