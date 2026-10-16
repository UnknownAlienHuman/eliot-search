//! Pure returned-pointer extent checks; no Win32 invocation or pointer dereference.
use super::*;

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
