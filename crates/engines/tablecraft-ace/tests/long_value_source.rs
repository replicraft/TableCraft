use std::io::{self, Cursor, Read, Seek, SeekFrom};
use tablecraft_ace::{
    Header, InspectError, LongTextReadLimits, LongValueReadLimits, LongValueReadSource,
    NativeReadSource,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

struct HookSource(Cursor<Vec<u8>>);

impl Read for HookSource {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.0.read(bytes)
    }
}

impl Seek for HookSource {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.0.seek(position)
    }
}

impl NativeReadSource for HookSource {
    fn qualify_header(&self, _header: Header) -> Result<Header, InspectError> {
        Err(InspectError::Unsupported)
    }

    fn decode_native_page(
        &mut self,
        number: u32,
        page: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), InspectError> {
        if cancelled() {
            return Err(InspectError::Cancelled);
        }
        budget.charge(Resource::Steps, 7)?;
        page.fill(number as u8);
        Ok(())
    }
}

#[test]
fn validated_storage_policies_cover_the_descriptor_and_keep_explicit_lower_limits() {
    let defaults = LongValueReadLimits::default();
    assert_eq!(defaults.maximum_bytes(), 0x3fff_ffff);
    for bytes in [1, 2_097_153, 0x3fff_ffff] {
        let limits = LongValueReadLimits::new(bytes).unwrap();
        assert_eq!(limits.maximum_bytes(), bytes);
    }
    for bytes in [0, 0x4000_0000, u32::MAX] {
        assert_eq!(
            LongValueReadLimits::new(bytes),
            Err(InspectError::InvalidLongValueLimits)
        );
    }
}

#[test]
fn outer_policy_and_mutable_reborrows_preserve_native_hooks_and_raw_bytes() {
    let mut bytes = vec![0; 12_288];
    bytes[..4].copy_from_slice(&[0, 1, 0, 0]);
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = 5;
    bytes[62..66].copy_from_slice(&[0xfb, 0x8a, 0xbc, 0x4e]);
    let header = Header::parse(&bytes[..66], bytes.len() as u64).unwrap();
    let limits = LongValueReadLimits::new(4_194_304).unwrap();
    let text_limits = LongTextReadLimits::new(4_194_304).unwrap();
    let mut source = LongValueReadSource::new(HookSource(Cursor::new(bytes.clone())), limits)
        .with_text_limits(text_limits);
    let mut borrowed = &mut source;
    let source = &mut borrowed;
    assert_eq!(source.long_value_limits(), limits);
    assert_eq!(source.long_text_limits(), text_limits);
    assert_eq!(
        source.qualify_header(header).err(),
        Some(InspectError::Unsupported)
    );

    let mut pool = ResourceBudget::new(0, 0, 20);
    let mut page = [0; 16];
    source
        .decode_native_page(9, &mut page, &mut pool, &|| false)
        .unwrap();
    assert_eq!(page, [9; 16]);
    assert_eq!(pool.remaining(Resource::Steps), 13);
    assert_eq!(
        source.decode_native_page(9, &mut page, &mut pool, &|| true),
        Err(InspectError::Cancelled)
    );

    let mut raw = Vec::new();
    source.read_to_end(&mut raw).unwrap();
    assert_eq!(raw, bytes);
    source.seek(SeekFrom::Start(20)).unwrap();
    assert_eq!(source.stream_position().unwrap(), 20);
}

#[test]
fn decoded_policies_validate_separately_and_survive_nested_storage_wrappers() {
    assert_eq!(LongTextReadLimits::default().maximum_bytes(), 0x7fff_fffe);
    for bytes in [1, 1_048_577, 0x7fff_fffe] {
        assert_eq!(
            LongTextReadLimits::new(bytes).unwrap().maximum_bytes(),
            bytes
        );
    }
    for bytes in [0, 0x7fff_ffff, u32::MAX] {
        assert_eq!(
            LongTextReadLimits::new(bytes),
            Err(InspectError::InvalidLongTextLimits)
        );
    }

    let text = LongTextReadLimits::new(3_145_728).unwrap();
    let inner = LongValueReadSource::new(
        Cursor::new(Vec::<u8>::new()),
        LongValueReadLimits::default(),
    )
    .with_text_limits(text);
    let storage = LongValueReadLimits::new(16_777_216).unwrap();
    let mut outer = LongValueReadSource::new(inner, storage);
    assert_eq!(outer.long_text_limits(), text);
    assert_eq!(outer.long_value_limits(), storage);
    let borrowed = &mut outer;
    assert_eq!(borrowed.long_text_limits(), text);

    let override_limit = LongTextReadLimits::new(1).unwrap();
    let outer = outer.with_text_limits(override_limit);
    assert_eq!(outer.long_text_limits(), override_limit);
    assert_eq!(outer.into_inner().long_text_limits(), text);
}
