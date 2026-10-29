//! Ties service processes to CHAMP's lifetime.
//!
//! On Windows every service process (Caddy, PHP, MySQL, PostgreSQL, ngrok) is
//! placed in a Job Object created with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.
//! CHAMP holds the only handle to that job, so when CHAMP exits for any reason
//! (normal quit, crash, Task Manager "End task", `tauri dev` rebuild, updater
//! restart) Windows closes the handle and terminates every process in the job.
//! Processes they spawn later (PostgreSQL backends, PHP workers) inherit the job.
//!
//! Only service processes are assigned. CHAMP itself is not in the job, so the
//! terminals and installers it opens are never killed with it.
//!
//! On other platforms this is a no-op; the explicit stop paths handle cleanup.

use std::process::Child;

#[cfg(target_os = "windows")]
mod imp {
    use std::os::windows::io::AsRawHandle;
    use std::process::Child;
    use std::sync::OnceLock;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
    };

    /// Job handle stored as an integer so the static is `Send + Sync`.
    /// `None` means the job could not be created; callers then fall back to
    /// the explicit stop and stale-process cleanup paths.
    static JOB: OnceLock<Option<usize>> = OnceLock::new();

    fn job_handle() -> Option<HANDLE> {
        let handle = JOB.get_or_init(|| {
            // SAFETY: plain Win32 calls with valid arguments; the handle is
            // intentionally kept open for the lifetime of the process.
            unsafe {
                let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if job.is_null() {
                    eprintln!("Warning: failed to create CHAMP job object");
                    return None;
                }
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let ok = SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const core::ffi::c_void,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                if ok == 0 {
                    eprintln!("Warning: failed to configure CHAMP job object");
                    CloseHandle(job);
                    return None;
                }
                Some(job as usize)
            }
        });
        handle.map(|value| value as HANDLE)
    }

    pub fn bind_child(child: &Child) -> bool {
        let Some(job) = job_handle() else {
            return false;
        };
        // SAFETY: the child handle stays valid while `child` is borrowed.
        unsafe { AssignProcessToJobObject(job, child.as_raw_handle() as HANDLE) != 0 }
    }

    pub fn bind_pid(pid: u32) -> bool {
        let Some(job) = job_handle() else {
            return false;
        };
        // SAFETY: the process handle is opened and closed locally.
        unsafe {
            let process = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid);
            if process.is_null() {
                return false;
            }
            let ok = AssignProcessToJobObject(job, process) != 0;
            CloseHandle(process);
            ok
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use std::process::Child;

    pub fn bind_child(_child: &Child) -> bool {
        false
    }

    pub fn bind_pid(_pid: u32) -> bool {
        false
    }
}

/// Make `child` terminate automatically when CHAMP exits. Best effort.
pub fn bind_child_to_app_lifetime(child: &Child) {
    if !imp::bind_child(child) && cfg!(target_os = "windows") {
        eprintln!(
            "Warning: could not tie process {} to CHAMP's lifetime",
            child.id()
        );
    }
}

/// Same as [`bind_child_to_app_lifetime`] for a process CHAMP adopted by PID
/// (a database server left running by an earlier CHAMP session).
pub fn bind_pid_to_app_lifetime(pid: u32) {
    if !imp::bind_pid(pid) && cfg!(target_os = "windows") {
        eprintln!("Warning: could not tie process {} to CHAMP's lifetime", pid);
    }
}
