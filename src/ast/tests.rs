
use super::*;

#[test]
fn system_v_record_layout() {
    // char, int: pad to 4, trailing pad to 8.
    let (off, size, align) = layout_record(&[(1, 1), (4, 4)]).unwrap();
    assert_eq!((off.as_slice(), size, align), (&[0, 4][..], 8, 4));
    // int, char: trailing padding.
    let (off, size, align) = layout_record(&[(4, 4), (1, 1)]).unwrap();
    assert_eq!((off.as_slice(), size, align), (&[0, 4][..], 8, 4));
    // char, long.
    let (off, size, align) = layout_record(&[(1, 1), (8, 8)]).unwrap();
    assert_eq!((off.as_slice(), size, align), (&[0, 8][..], 16, 8));
    // char, short.
    let (off, size, align) = layout_record(&[(1, 1), (2, 2)]).unwrap();
    assert_eq!((off.as_slice(), size, align), (&[0, 2][..], 4, 2));
    // long[2] as a field is aligned to 8, not 16, even though it is 16 bytes.
    let (off, size, align) = layout_record(&[(1, 1), (16, 8)]).unwrap();
    assert_eq!((off.as_slice(), size, align), (&[0, 8][..], 24, 8));
    // char[16] as a field stays alignment 1.
    let (off, size, align) = layout_record(&[(1, 1), (16, 1)]).unwrap();
    assert_eq!((off.as_slice(), size, align), (&[0, 1][..], 17, 1));
    // nested struct of align 4 after a char.
    let (off, size, align) = layout_record(&[(1, 1), (8, 4)]).unwrap();
    assert_eq!((off.as_slice(), size, align), (&[0, 4][..], 12, 4));
}

fn bits(size: u64, align: u32, width: u32, named: bool) -> LayoutMember {
    LayoutMember {
        size,
        align,
        bit_width: Some(width),
        named,
    }
}

fn plain(size: u64, align: u32) -> LayoutMember {
    LayoutMember {
        size,
        align,
        bit_width: None,
        named: true,
    }
}

fn laid(members: &[LayoutMember]) -> (u64, u32, Vec<(u64, u32)>) {
    let (places, size, align) = layout_members(members, false).unwrap();
    let bits = places
        .iter()
        .map(|place| (place.offset, place.bit_offset))
        .collect();
    (size, align, bits)
}

#[test]
fn bitfield_layout_matches_the_psabi() {
    // int a:3; int b:5. Both live in the low byte. Alignment comes from int.
    assert_eq!(
        laid(&[bits(4, 4, 3, true), bits(4, 4, 5, true)]),
        (4, 4, vec![(0, 0), (0, 3)])
    );
    // char a:4; int b:4. The int fits in the same 4-byte unit.
    assert_eq!(
        laid(&[bits(1, 1, 4, true), bits(4, 4, 4, true)]),
        (4, 4, vec![(0, 0), (0, 4)])
    );
    // int a:3; char b. The char starts at the next byte.
    assert_eq!(
        laid(&[bits(4, 4, 3, true), plain(1, 1)]),
        (4, 4, vec![(0, 0), (1, 0)])
    );
    // char a; int b:5. The bit-field continues inside the int unit at bit 0.
    assert_eq!(
        laid(&[plain(1, 1), bits(4, 4, 5, true)]),
        (4, 4, vec![(0, 0), (0, 8)])
    );
    // int a:20; int b:20. The second does not fit and starts a new int.
    assert_eq!(
        laid(&[bits(4, 4, 20, true), bits(4, 4, 20, true)]),
        (8, 4, vec![(0, 0), (4, 32)])
    );
    // int a:3; int :0; int b:3.
    assert_eq!(
        laid(&[
            bits(4, 4, 3, true),
            bits(4, 4, 0, false),
            bits(4, 4, 3, true)
        ]),
        (8, 4, vec![(0, 0), (4, 32), (4, 32)])
    );
    // char a:4; char b:5. The second char starts a new byte.
    assert_eq!(
        laid(&[bits(1, 1, 4, true), bits(1, 1, 5, true)]),
        (2, 1, vec![(0, 0), (1, 8)])
    );
    // int a:17; char b:3. The char starts at bit 17.
    assert_eq!(
        laid(&[bits(4, 4, 17, true), bits(1, 1, 3, true)]),
        (4, 4, vec![(0, 0), (2, 17)])
    );
    // char a:3; int :0; char b. The zero-width field aligns, but does not
    // raise the struct alignment, so the size stays 5.
    assert_eq!(
        laid(&[bits(1, 1, 3, true), bits(4, 4, 0, false), plain(1, 1)]),
        (5, 1, vec![(0, 0), (4, 32), (4, 0)])
    );
    // int :3; char a. Unnamed, so the alignment stays 1 and the char is
    // in the next byte.
    assert_eq!(
        laid(&[bits(4, 4, 3, false), plain(1, 1)]),
        (2, 1, vec![(0, 0), (1, 0)])
    );
    // int :3 alone is one byte. The unnamed field does not align the struct.
    assert_eq!(laid(&[bits(4, 4, 3, false)]), (1, 1, vec![(0, 0)]));
    // int :0 alone reserves nothing.
    assert_eq!(laid(&[bits(4, 4, 0, false)]), (0, 1, vec![(0, 0)]));
    // char a:5; int b:30. 30 bits do not fit in the remaining int unit.
    assert_eq!(
        laid(&[bits(1, 1, 5, true), bits(4, 4, 30, true)]),
        (8, 4, vec![(0, 0), (4, 32)])
    );
    // int a:3; long b:3. The long fits in the 8-byte unit that starts at 0.
    assert_eq!(
        laid(&[bits(4, 4, 3, true), bits(8, 8, 3, true)]),
        (8, 8, vec![(0, 0), (0, 3)])
    );
}

#[test]
fn struct_identity_is_the_declaration() {
    let a = StructRef::new_incomplete(1, "S", RecordKind::Struct);
    let b = StructRef::new_incomplete(2, "S", RecordKind::Struct);
    assert_ne!(a, b);
    assert_eq!(a.clone(), a);
    assert!(!a.is_complete());
    a.complete(
        vec![StructField::plain("x", CType::int_type(), 0)],
        4,
        4,
        "struct.S".to_string(),
    );
    assert!(a.is_complete());
    assert_eq!(a.size(), Some(4));
    assert_eq!(a.field("x").unwrap().0, 0);
    assert!(a.field("y").is_none());
    assert_eq!(CType::Struct(a.clone()).display(), "struct S");
    assert_eq!(CType::Struct(a).ir_type(), "%struct.S");
}

#[test]
fn convert_bits_extends_to_128() {
    use super::{convert_bits, TypeName};
    // `__uint128_t a = -1;` keeps 128 one-bits.
    let all_ones = convert_bits(u128::from((-1i32) as u32), TypeName::Int, TypeName::UInt128);
    assert_eq!(all_ones, u128::MAX);
    // `__uint128_t b = 0xffffffffffffffffULL;` zero-extends.
    let ull_max = convert_bits(u64::MAX as u128, TypeName::ULongLong, TypeName::UInt128);
    assert_eq!(ull_max, u64::MAX as u128);
    // signed `__int128` from -1 is also all ones.
    let signed = convert_bits(u128::from((-1i32) as u32), TypeName::Int, TypeName::Int128);
    assert_eq!(signed, u128::MAX);
    // Narrowing drops the high half.
    assert_eq!(
        convert_bits(u128::MAX, TypeName::UInt128, TypeName::ULongLong),
        u64::MAX as u128
    );
}

#[test]
fn signed_shl_fits_rejects_a_lost_magnitude_bit() {
    use super::signed_shl_fits;
    assert!(signed_shl_fits(1, 126, 128));
    assert!(!signed_shl_fits(1 << 126, 2, 128));
    assert!(!signed_shl_fits(1, 127, 128));
    assert!(signed_shl_fits(1, 30, 32));
    assert!(!signed_shl_fits(1, 31, 32));
    assert!(!signed_shl_fits(-1, 1, 128));
}
