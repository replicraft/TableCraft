//! Explicit current-user private sidecars; inherited defaults never qualify.
//! Uses documented TOKEN_USER, absolute creation SD and self-relative query SD.
use super::{handle, last_error};
use crate::IoError;
use std::{
    fs::File,
    io::ErrorKind,
    mem::{offset_of, size_of, size_of_val},
    os::windows::io::{FromRawHandle, OwnedHandle},
    ptr::{copy_nonoverlapping, null_mut, read_unaligned},
};
use windows_sys::Win32::Foundation::{ERROR_NO_TOKEN, GetLastError, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Security::{
    ACCESS_ALLOWED_ACE, ACL, ACL_REVISION, AddAccessAllowedAceEx, DACL_SECURITY_INFORMATION,
    EqualSid, GetAce, GetKernelObjectSecurity, GetLengthSid, GetSecurityDescriptorControl,
    GetSecurityDescriptorDacl, GetSecurityDescriptorOwner, GetTokenInformation, InitializeAcl,
    InitializeSecurityDescriptor, IsValidSid, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR,
    PSID, SE_DACL_DEFAULTED, SE_DACL_PRESENT, SE_DACL_PROTECTED, SE_SELF_RELATIVE,
    SECURITY_DESCRIPTOR, SECURITY_DESCRIPTOR_RELATIVE, SECURITY_MAX_SID_SIZE,
    SetSecurityDescriptorControl, SetSecurityDescriptorDacl, SetSecurityDescriptorOwner,
    TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows_sys::Win32::Storage::FileSystem::FILE_ALL_ACCESS;
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentThread, OpenProcessToken, OpenThreadToken,
};

const SID_BYTES: usize = SECURITY_MAX_SID_SIZE as usize;
const SID_HEADER_BYTES: usize = 8;
const SID_SUBAUTHORITY_BYTES: usize = size_of::<u32>();
const ACL_BYTES: usize = size_of::<ACL>() + offset_of!(ACCESS_ALLOWED_ACE, SidStart) + SID_BYTES;
const DESCRIPTOR_BYTES: usize = 8192;
const TOKEN_BUFFER_BYTES: usize =
    (size_of::<TOKEN_USER>() + SID_BYTES).div_ceil(size_of::<usize>()) * size_of::<usize>();
const DESCRIPTOR_REVISION: u32 = 1;
const ALLOW_ACE_TYPE: u8 = 0;

/// Caller admission covers all bounded stack buffers and API inspection passes.
pub(super) const WORK_BYTES: u64 = (DESCRIPTOR_BYTES
    + ACL_BYTES
    + SID_BYTES
    + TOKEN_BUFFER_BYTES
    + size_of::<TOKEN_USER>()
    + size_of::<SECURITY_DESCRIPTOR>()) as u64;
pub(super) const WORK_STEPS: u64 = 32 + WORK_BYTES * 4;

pub(super) struct PrivateSecurity {
    sid: [u32; SID_BYTES.div_ceil(size_of::<u32>())],
}

fn denied() -> IoError {
    IoError::Io(ErrorKind::PermissionDenied)
}

fn contains(base: usize, length: usize, pointer: usize, bytes: usize) -> bool {
    pointer
        .checked_sub(base)
        .is_some_and(|offset| offset <= length && bytes <= length - offset)
}

// Only call with an OS-returned pointer into the stated live writable buffer.
fn sid_in_buffer(sid: PSID, base: usize, length: usize) -> bool {
    let address = sid as usize;
    if !contains(base, length, address, SID_HEADER_BYTES) {
        return false;
    }
    // SAFETY: the fixed SID header lies within the bounded live buffer above.
    let count = unsafe { *sid.cast::<u8>().add(1) } as usize;
    let bytes = SID_HEADER_BYTES + count * SID_SUBAUTHORITY_BYTES;
    if bytes > SID_BYTES || !contains(base, length, address, bytes) {
        return false;
    }
    // SAFETY: the entire encoded SID, including each subauthority, is in bounds.
    unsafe { IsValidSid(sid) != 0 && GetLengthSid(sid) as usize == bytes }
}

impl PrivateSecurity {
    pub(super) fn capture() -> Result<Self, IoError> {
        let mut raw = null_mut();
        // SAFETY: current-thread pseudo handle and initialized output pointer;
        // request only query rights. Impersonation identity takes precedence.
        if unsafe { OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, 1, &mut raw) } == 0 {
            // SAFETY: immediate thread-local error observation after failed call.
            if unsafe { GetLastError() } != ERROR_NO_TOKEN {
                return Err(last_error());
            }
            // SAFETY: no thread token exists; current-process pseudo handle and
            // initialized output are valid for the documented fallback.
            if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw) } == 0 {
                return Err(last_error());
            }
        }
        if raw.is_null() || raw == INVALID_HANDLE_VALUE {
            return Err(denied());
        }
        // SAFETY: successful token open returned a uniquely owned valid handle.
        let token = unsafe { OwnedHandle::from_raw_handle(raw) };
        let mut buffer = [0usize; TOKEN_BUFFER_BYTES / size_of::<usize>()];
        let mut length = 0;
        // SAFETY: aligned bounded TOKEN_USER buffer; token is retained throughout.
        if unsafe {
            GetTokenInformation(
                raw,
                TokenUser,
                buffer.as_mut_ptr().cast(),
                size_of_val(&buffer) as u32,
                &mut length,
            )
        } == 0
        {
            return Err(last_error());
        }
        if (length as usize) < size_of::<TOKEN_USER>() || length as usize > size_of_val(&buffer) {
            return Err(denied());
        }
        // SAFETY: successful query initialized at least one full TOKEN_USER.
        let user = unsafe { read_unaligned(buffer.as_ptr().cast::<TOKEN_USER>()) };
        if !sid_in_buffer(user.User.Sid, buffer.as_ptr() as usize, length as usize) {
            return Err(denied());
        }
        let mut result = Self {
            sid: [0; SID_BYTES.div_ceil(size_of::<u32>())],
        };
        // SAFETY: verified live source SID and destination capacity SECURITY_MAX_SID_SIZE.
        unsafe {
            copy_nonoverlapping(
                user.User.Sid.cast::<u8>(),
                result.sid.as_mut_ptr().cast(),
                GetLengthSid(user.User.Sid) as usize,
            );
        }
        drop(token);
        Ok(result)
    }

    pub(super) fn with_descriptor<T>(
        &mut self,
        operation: impl FnOnce(PSECURITY_DESCRIPTOR) -> T,
    ) -> Result<T, IoError> {
        let mut acl = [0u32; ACL_BYTES.div_ceil(size_of::<u32>())];
        let acl_pointer = acl.as_mut_ptr().cast::<ACL>();
        let mut descriptor = SECURITY_DESCRIPTOR::default();
        let pointer = (&mut descriptor as *mut SECURITY_DESCRIPTOR).cast();
        // SAFETY: all absolute SD components are aligned, bounded and retained
        // through operation; its single explicit ACE has no inheritance flags.
        let success = unsafe {
            InitializeSecurityDescriptor(pointer, DESCRIPTOR_REVISION) != 0
                && InitializeAcl(acl_pointer, size_of_val(&acl) as u32, ACL_REVISION) != 0
                && AddAccessAllowedAceEx(
                    acl_pointer,
                    ACL_REVISION,
                    0,
                    FILE_ALL_ACCESS,
                    self.sid.as_mut_ptr().cast(),
                ) != 0
                && SetSecurityDescriptorOwner(pointer, self.sid.as_mut_ptr().cast(), 0) != 0
                && SetSecurityDescriptorDacl(pointer, 1, acl_pointer, 0) != 0
                && SetSecurityDescriptorControl(pointer, SE_DACL_PROTECTED, SE_DACL_PROTECTED) != 0
        };
        if !success {
            return Err(last_error());
        }
        Ok(operation(pointer))
    }

    pub(super) fn verify(&mut self, file: &File) -> Result<(), IoError> {
        let mut buffer = [0usize; DESCRIPTOR_BYTES.div_ceil(size_of::<usize>())];
        let pointer = buffer.as_mut_ptr().cast();
        let mut length = 0;
        // SAFETY: file has READ_CONTROL access, output is aligned and bounded;
        // this API returns a self-relative SD into this caller-owned buffer.
        if unsafe {
            GetKernelObjectSecurity(
                handle(file),
                OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                pointer,
                size_of_val(&buffer) as u32,
                &mut length,
            )
        } == 0
        {
            return Err(last_error());
        }
        if length as usize > size_of_val(&buffer)
            || (length as usize) < size_of::<SECURITY_DESCRIPTOR_RELATIVE>()
        {
            return Err(denied());
        }
        let mut control = 0;
        let mut revision = 0;
        let mut owner = null_mut();
        let mut owner_defaulted = 0;
        let mut dacl = null_mut();
        let mut present = 0;
        let mut defaulted = 0;
        // SAFETY: successful kernel query supplied the live self-relative SD;
        // these APIs return borrowed pointers into that same retained buffer.
        let success = unsafe {
            GetSecurityDescriptorControl(pointer, &mut control, &mut revision) != 0
                && GetSecurityDescriptorOwner(pointer, &mut owner, &mut owner_defaulted) != 0
                && GetSecurityDescriptorDacl(pointer, &mut present, &mut dacl, &mut defaulted) != 0
        };
        let required = SE_DACL_PRESENT | SE_DACL_PROTECTED | SE_SELF_RELATIVE;
        if !success
            || revision != DESCRIPTOR_REVISION
            || control & required != required
            || control & SE_DACL_DEFAULTED != 0
            || owner_defaulted != 0
            || present == 0
            || defaulted != 0
            || !sid_in_buffer(owner, buffer.as_ptr() as usize, length as usize)
            || !contains(
                buffer.as_ptr() as usize,
                length as usize,
                dacl as usize,
                size_of::<ACL>(),
            )
        {
            return Err(denied());
        }
        // SAFETY: validated owner SID and retained captured current-user SID.
        if unsafe { EqualSid(owner, self.sid.as_mut_ptr().cast()) } == 0 {
            return Err(denied());
        }
        // SAFETY: complete ACL header lies within the returned bounded SD.
        let header = unsafe { read_unaligned(dacl) };
        if header.AclRevision as u32 != ACL_REVISION
            || header.AceCount != 1
            || (header.AclSize as usize)
                < size_of::<ACL>() + offset_of!(ACCESS_ALLOWED_ACE, SidStart) + SID_HEADER_BYTES
            || header.AclSize as usize > ACL_BYTES
            || !contains(
                buffer.as_ptr() as usize,
                length as usize,
                dacl as usize,
                header.AclSize as usize,
            )
        {
            return Err(denied());
        }
        let mut ace = null_mut();
        // SAFETY: kernel-returned ACL is fully bounded above; index zero exists.
        if unsafe { GetAce(dacl, 0, &mut ace) } == 0 {
            return Err(last_error());
        }
        if !contains(
            dacl as usize,
            header.AclSize as usize,
            ace as usize,
            size_of::<ACCESS_ALLOWED_ACE>(),
        ) {
            return Err(denied());
        }
        // SAFETY: complete ordinary ACE header was bounded above.
        let allowed = unsafe { read_unaligned(ace.cast::<ACCESS_ALLOWED_ACE>()) };
        let sid = (ace as usize + offset_of!(ACCESS_ALLOWED_ACE, SidStart)) as PSID;
        if allowed.Header.AceType != ALLOW_ACE_TYPE
            || allowed.Header.AceFlags != 0
            || allowed.Mask != FILE_ALL_ACCESS
            || !contains(
                dacl as usize,
                header.AclSize as usize,
                ace as usize,
                allowed.Header.AceSize as usize,
            )
            || !sid_in_buffer(sid, ace as usize, allowed.Header.AceSize as usize)
        {
            return Err(denied());
        }
        // SAFETY: both SIDs are complete and retained; require exactly one SID
        // within this ACE, excluding extra callback data or malformed padding.
        if unsafe {
            allowed.Header.AceSize as usize
                != offset_of!(ACCESS_ALLOWED_ACE, SidStart) + GetLengthSid(sid) as usize
                || EqualSid(sid, self.sid.as_mut_ptr().cast()) == 0
        } {
            return Err(denied());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
