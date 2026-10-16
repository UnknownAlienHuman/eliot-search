//! Pure returned-pointer extent checks; no Win32 invocation or pointer dereference.
use super::*;

struct CopyContext;
impl StorageSecurityContext for CopyContext {
    type Error = ();
    fn checkpoint(&mut self) -> Result<(), ()> {
        Ok(())
    }
    fn charge(&mut self, _: Work) -> Result<(), ()> {
        Ok(())
    }
}

#[test]
fn semantic_copy_never_borrows_uninitialized_descriptor_or_acl_slack() {
    use core::mem::MaybeUninit;
    let mut allocation = [MaybeUninit::<u8>::uninit(); 128];
    let base = allocation.as_mut_ptr().cast::<u8>();
    let put = |offset: usize, bytes: &[u8]| {
        // SAFETY: these writes initialize only the meaningful SDK fields below.
        unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), base.add(offset), bytes.len()) };
    };
    put(0, &[1]); // reserved/group/SACL fields intentionally remain uninitialized
    put(2, &0x8004_u16.to_le_bytes());
    put(4, &20_u32.to_le_bytes());
    put(16, &36_u32.to_le_bytes());
    put(20, SYSTEM);
    put(36, &[2]);
    put(38, &64_u16.to_le_bytes());
    put(40, &1_u16.to_le_bytes());
    put(44, &[0, 3]);
    put(46, &20_u16.to_le_bytes());
    put(48, &FILE_ALL_ACCESS.to_le_bytes());
    put(52, SYSTEM);
    // SAFETY: every scalar/SID/used-ACE field read by the copier is initialized;
    // all other bytes deliberately model native allocation and ACL free capacity.
    let (owner, control, acl) = unsafe {
        descriptor_parts(
            base.cast(),
            allocation.len(),
            base.add(20).cast(),
            base.add(36).cast(),
            &mut CopyContext,
        )
        .unwrap()
    };
    assert_eq!(owner, SYSTEM);
    assert_eq!(control, 0x8004);
    assert_eq!(acl.len(), 64);
    assert_eq!(&acl[16..28], SYSTEM);
    assert!(acl[28..].iter().all(|byte| *byte == 0));
    assert_eq!((acl[1], acl[6], acl[7]), (0, 0, 0));
    qualify::<()>(
        &owner,
        control,
        &acl,
        search_os_secrets::hash_native_revision_user_sid(SYSTEM),
    )
    .unwrap();
}

#[test]
fn relative_components_must_match_aligned_in_allocation_addresses() {
    let allocation = [0_u32; 24];
    let base = allocation.as_ptr().cast_mut().cast::<u8>();
    let bytes = core::mem::size_of_val(&allocation);
    assert_eq!(
        checked_offset::<()>(base.cast(), bytes, 20, base.wrapping_add(20).cast()).unwrap(),
        20
    );
    for (offset, pointer) in [
        (0, base),
        (16, base.wrapping_add(16)),
        (21, base.wrapping_add(21)),
        (96, base.wrapping_add(96)),
        (20, base.wrapping_add(24)),
        (20, null_mut()),
        (u32::MAX, base),
    ] {
        assert!(matches!(
            checked_offset::<()>(base.cast(), bytes, offset, pointer.cast()),
            Err(Error::InvalidDescriptor)
        ));
    }
}
