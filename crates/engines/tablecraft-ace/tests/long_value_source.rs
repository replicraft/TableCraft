use std::io::{self, Cursor, Read, Seek, SeekFrom};
use tablecraft_ace::{
    Header, InspectError, LongValueReadLimits, LongValueReadSource, NativeReadSource,
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
fn validated_policies_preserve_defaults_and_bound_reference_counts() {
    let defaults = LongValueReadLimits::default();
    assert_eq!(defaults.maximum_bytes(), 2_097_152);
    assert_eq!(defaults.maximum_chunks(), 1024);
    for (bytes, chunks) in [(1, 1024), (2_097_153, 2048), (33_554_432, 16_384)] {
        let limits = LongValueReadLimits::new(bytes).unwrap();
        assert_eq!(limits.maximum_bytes(), bytes);
        assert_eq!(limits.maximum_chunks(), chunks);
    }
    for bytes in [0, 33_554_433, u32::MAX] {
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
    let mut source = LongValueReadSource::new(HookSource(Cursor::new(bytes.clone())), limits);
    let mut borrowed = &mut source;
    let source = &mut borrowed;
    assert_eq!(source.long_value_limits(), limits);
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
