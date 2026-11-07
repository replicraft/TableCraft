//! Build an independently broader explicit DACL through actual Windows APIs.
use super::*;
use crate::windows_fs::{self, Location};
use std::fs;
use tablecraft_core::limits::ResourceBudget;
use windows_sys::Wdk::Storage::FileSystem::FILE_CREATE;
use windows_sys::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE};
use windows_sys::Win32::Storage::FileSystem::{DELETE, FILE_GENERIC_READ, SYNCHRONIZE};

#[test]
fn windows_private_security_rejects_additional_explicit_aces_even_for_same_owner() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("noncanonical-private-journal");
    let mut work = ResourceBudget::new(1_000_000, 0, 1_000_000);
    let mut location = Location::capture(&path, &mut work, &|| false).unwrap();
    let mut security = PrivateSecurity::capture().unwrap();
    let mut sid = security.sid;
    let mut acl = [0u32;
        (size_of::<ACL>() + 2 * (offset_of!(ACCESS_ALLOWED_ACE, SidStart) + SID_BYTES))
            .div_ceil(size_of::<u32>())];
    let pointer = acl.as_mut_ptr().cast::<ACL>();
    // SAFETY: both ACEs refer to the complete captured SID; the ACL buffer has
    // room for two maximal ordinary ACEs. No privilege or external identity used.
    assert!(unsafe {
        InitializeAcl(pointer, size_of_val(&acl) as u32, ACL_REVISION) != 0
            && AddAccessAllowedAceEx(
                pointer,
                ACL_REVISION,
                0,
                FILE_ALL_ACCESS,
                sid.as_mut_ptr().cast(),
            ) != 0
            && AddAccessAllowedAceEx(
                pointer,
                ACL_REVISION,
                0,
                FILE_GENERIC_READ,
                sid.as_mut_ptr().cast(),
            ) != 0
    });
    let file = security
        .with_descriptor(|descriptor| {
            // SAFETY: absolute descriptor and separate bounded ACL remain live
            // until the synchronous relative creation completes inside this closure.
            assert!(unsafe { SetSecurityDescriptorDacl(descriptor, 1, pointer, 0) } != 0);
            windows_fs::relative_file(
                &location.parent,
                &mut location.name,
                descriptor,
                FILE_CREATE,
                GENERIC_READ | GENERIC_WRITE | DELETE | SYNCHRONIZE,
            )
        })
        .unwrap()
        .unwrap_or_else(|failure| panic!("DACL fixture creation failed: {}", failure.cause));
    let mut retained = Some(file);
    assert!(security.verify(retained.as_ref().unwrap()).is_err());
    windows_fs::close_file(&mut retained).unwrap();
    assert!(windows_fs::open_private_write_through(&location.parent, &mut location.name).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"");
}

#[test]
fn windows_security_sid_checks_reject_outside_header_and_subauthority_bounds() {
    let mut storage = [0u32; SID_BYTES.div_ceil(size_of::<u32>())];
    let base = storage.as_mut_ptr() as usize;
    let sid = storage.as_mut_ptr().cast();
    assert!(!sid_in_buffer(null_mut(), base, SID_BYTES));
    assert!(!sid_in_buffer((base - 1) as PSID, base, SID_BYTES));
    assert!(!sid_in_buffer((base + SID_BYTES) as PSID, base, SID_BYTES));
    assert!(!sid_in_buffer(sid, base, SID_HEADER_BYTES - 1));

    // Revision 1, one subauthority: its full 12-byte payload must be retained.
    storage[0] = u32::from_le_bytes([1, 1, 0, 0]);
    let sid = storage.as_mut_ptr().cast();
    assert!(!sid_in_buffer(sid, base, SID_HEADER_BYTES));
    assert!(!sid_in_buffer(
        sid,
        base,
        SID_HEADER_BYTES + SID_SUBAUTHORITY_BYTES - 1
    ));

    // Sixteen subauthorities exceed the documented maximal SID buffer even
    // when the header itself fits, so no Windows SID API receives this pointer.
    storage[0] = u32::from_le_bytes([1, 16, 0, 0]);
    let sid = storage.as_mut_ptr().cast();
    assert!(!sid_in_buffer(sid, base, SID_BYTES));
}
