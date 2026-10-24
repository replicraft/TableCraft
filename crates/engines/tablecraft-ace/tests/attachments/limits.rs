use super::*;

fn large_budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 0, 100_000_000)
}

#[test]
fn explicit_file_allowances_admit_exact_boundaries_without_changing_defaults() {
    for (size, compressed) in [(1_500_000, false), (3_145_728, true), (16_777_216, true)] {
        let data = vec![0x5a; size];
        let bytes = encoded("bin", &data, compressed);
        let limits = AttachmentDecodeLimits::new(size as u32).unwrap();
        let decoded =
            decode_attachment_with_limits(&bytes, limits, &mut large_budget(), &|| false).unwrap();
        assert_eq!(decoded.data(), data);
        assert_eq!(decoded.type_label(), "bin");
        assert_eq!(
            decode_attachment(&bytes, &mut large_budget(), &|| false).err(),
            Some(InspectError::StructureLimit)
        );
        let smaller = AttachmentDecodeLimits::new(size as u32 - 1).unwrap();
        assert_eq!(
            decode_attachment_with_limits(&bytes, smaller, &mut large_budget(), &|| false).err(),
            Some(InspectError::StructureLimit)
        );
    }
}

#[test]
fn limits_are_validated_and_encoded_header_ceilings_remain_separate() {
    assert_eq!(
        AttachmentDecodeLimits::default().maximum_file_bytes(),
        1_048_576
    );
    for size in [0, 16_777_217, u32::MAX] {
        assert_eq!(
            AttachmentDecodeLimits::new(size),
            Err(InspectError::InvalidAttachmentLimits)
        );
    }
    let limits = AttachmentDecodeLimits::new(4_194_304).unwrap();
    let encoded_too_large = encoded("bin", &vec![42; 2_097_152], false);
    assert_eq!(
        decode_attachment_with_limits(&encoded_too_large, limits, &mut large_budget(), &|| false)
            .err(),
        Some(InspectError::StructureLimit)
    );
    let header_too_large = encoded(&"t".repeat(3000), &[], false);
    assert_eq!(
        decode_attachment_with_limits(&header_too_large, limits, &mut large_budget(), &|| false)
            .err(),
        Some(InspectError::Malformed)
    );
}

#[test]
fn large_file_sessions_keep_checksums_exact_lengths_and_explicit_recovery() {
    let original = encoded("bin", &vec![0x51; 1_100_000], true);
    let mut checksum = original.clone();
    *checksum.last_mut().unwrap() ^= 1;
    let mut trailing = original.clone();
    trailing.push(0);
    let mut short_length = original.clone();
    short_length[4..8].copy_from_slice(&1_000_000_u32.to_le_bytes());
    let limits = AttachmentDecodeLimits::new(1_100_000).unwrap();
    let mut pool = large_budget();
    let mut decoder = AttachmentDecoder::new_with_limits(&mut pool, &|| false, limits);

    for invalid in [checksum, trailing, short_length] {
        assert_eq!(
            decoder.decode(&invalid).err(),
            Some(InspectError::Malformed)
        );
        assert_eq!(
            decoder
                .decode(&encoded("small", &[1, 2, 3], true))
                .unwrap()
                .data(),
            [1, 2, 3]
        );
    }
    assert_eq!(decoder.decode(&original).unwrap().data().len(), 1_100_000);
}

#[test]
fn larger_file_limits_do_not_raise_the_cumulative_allocation_pool() {
    let original = encoded("bin", &vec![42; 1_100_000], true);
    let limits = AttachmentDecodeLimits::new(1_100_000).unwrap();
    let mut pool = ResourceBudget::new(1_000_000, 0, 100_000_000);
    assert!(matches!(
        decode_attachment_with_limits(&original, limits, &mut pool, &|| false),
        Err(InspectError::Limit(error)) if error.resource == Resource::Bytes
    ));
}

#[test]
fn sampled_large_stream_checkpoints_propagate_cancellation() {
    let original = encoded("bin", &vec![42; 1_100_000], true);
    let limits = AttachmentDecodeLimits::new(1_100_000).unwrap();
    let checks = Cell::new(0);
    let count = || {
        checks.set(checks.get() + 1);
        false
    };
    decode_attachment_with_limits(&original, limits, &mut large_budget(), &count).unwrap();
    let observed = checks.get();
    assert!(observed > 32);

    for stop in [1, 2, 8, 32, observed / 2, observed - 1, observed] {
        let checks = Cell::new(0);
        let cancelled = || {
            checks.set(checks.get() + 1);
            checks.get() >= stop
        };
        assert_eq!(
            decode_attachment_with_limits(&original, limits, &mut large_budget(), &cancelled).err(),
            Some(InspectError::Cancelled)
        );
    }
}

#[test]
fn a_large_allowance_does_not_preallocate_large_files_or_codec_state() {
    let limits = AttachmentDecodeLimits::new(16_777_216).unwrap();
    let mut pool = ResourceBudget::new(192 * 1024, 0, 1_000_000);
    let mut decoder = AttachmentDecoder::new_with_limits(&mut pool, &|| false, limits);
    assert_eq!(decoder.budget_mut().remaining(Resource::Bytes), 192 * 1024);

    for byte in 0_u8..32 {
        assert_eq!(
            decoder
                .decode(&encoded("bin", &[byte; 8], true))
                .unwrap()
                .data(),
            [byte; 8]
        );
    }
    assert!(decoder.budget_mut().remaining(Resource::Bytes) > 32 * 1024);
}

#[test]
fn larger_encoded_envelopes_require_an_independent_explicit_allowance() {
    let data = vec![42; 3_145_728];
    let bytes = encoded("bin", &data, false);
    let file_limits = AttachmentDecodeLimits::new(3_145_728).unwrap();
    assert_eq!(file_limits.maximum_encoded_bytes(), 2_097_152);
    assert_eq!(
        decode_attachment_with_limits(&bytes, file_limits, &mut large_budget(), &|| false).err(),
        Some(InspectError::StructureLimit)
    );
    let limits = file_limits.with_encoded_bytes(bytes.len() as u32).unwrap();
    assert_eq!(
        decode_attachment_with_limits(&bytes, limits, &mut large_budget(), &|| false)
            .unwrap()
            .data(),
        data
    );
    let short = limits.with_encoded_bytes(bytes.len() as u32 - 1).unwrap();
    assert_eq!(
        decode_attachment_with_limits(&bytes, short, &mut large_budget(), &|| false).err(),
        Some(InspectError::StructureLimit)
    );
    for size in [0, 33_554_433, u32::MAX] {
        assert_eq!(
            file_limits.with_encoded_bytes(size),
            Err(InspectError::InvalidAttachmentLimits)
        );
    }
    let file_too_small = AttachmentDecodeLimits::default()
        .with_encoded_bytes(4_194_304)
        .unwrap();
    assert_eq!(
        decode_attachment_with_limits(&bytes, file_too_small, &mut large_budget(), &|| false).err(),
        Some(InspectError::StructureLimit)
    );
}
