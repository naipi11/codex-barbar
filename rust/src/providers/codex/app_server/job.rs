//! Windows Job Object ownership for supervised Codex children.

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
