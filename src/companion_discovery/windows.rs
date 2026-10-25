use super::*;
use crate::companion_ipc::windows::{self as ipc, Descriptor, Owned};
use std::ptr::null_mut;
use windows_sys::Win32::{
    Foundation::*, Security::Authorization::*, Security::*, System::Threading::*,
};
pub(super) fn default_root() -> io::Result<PathBuf> {
    ipc::endpoint()
}
pub(super) struct Guard {
    mutex: Owned,
    endpoint: PathBuf,
    _same_thread: std::marker::PhantomData<std::rc::Rc<()>>,
}
impl Guard {
    pub(super) fn acquire(endpoint: &Path) -> io::Result<Self> {
        // Tests supply unique pipe names; production uses the current SID digest.
        ipc::parent(endpoint)?;
        let suffix = endpoint.to_str().unwrap().rsplit('\\').next().unwrap();
        let name: Vec<u16> = format!("Local\\{suffix}-launch\0").encode_utf16().collect();
        let descriptor = Descriptor::new()?;
        let attributes = descriptor.attributes();
        let handle = unsafe { CreateMutexW(&attributes, 0, name.as_ptr()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let mutex = Owned(handle);
        let user = crate::windows_fs::user()?;
        // Refuse a pre-existing object belonging to another account. Never replace its DACL.
        let (mut owner, mut sd) = (null_mut(), null_mut());
        let status = unsafe {
            GetSecurityInfo(
                handle,
                SE_KERNEL_OBJECT,
                OWNER_SECURITY_INFORMATION,
                &mut owner,
                null_mut(),
                null_mut(),
                null_mut(),
                &mut sd,
            )
        };
        if status != 0 {
            return Err(io::Error::from_raw_os_error(status as i32));
        }
        let same = unsafe { EqualSid(owner, crate::windows_fs::sid(&user)) != 0 };
        unsafe {
            LocalFree(sd);
        }
        if !same {
            return Err(invalid("launch mutex owner mismatch"));
        }
        let wait = unsafe { WaitForSingleObject(handle, 0) };
        if wait == WAIT_TIMEOUT {
            return Err(io::Error::from(io::ErrorKind::WouldBlock));
        }
        if wait != WAIT_OBJECT_0 && wait != WAIT_ABANDONED {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            mutex,
            endpoint: endpoint.into(),
            _same_thread: std::marker::PhantomData,
        })
    }
    pub(super) fn existing(&self) -> io::Result<Option<PathBuf>> {
        Ok(Some(self.endpoint.clone()))
    }
    pub(super) fn next(&self) -> io::Result<PathBuf> {
        Ok(self.endpoint.clone())
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        unsafe {
            ReleaseMutex(self.mutex.0);
        }
    }
}
