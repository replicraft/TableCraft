use super::*;
use flate2::{Compression, write::ZlibEncoder};
use std::io::Write;
use tablecraft_ace::AttachmentDecodeLimits;
use tablecraft_core::limits::Resource;

fn large_encoded() -> Vec<u8> {
    // Independent complete wrapper with an empty type and 1,100,000 file bytes.
    let mut body = 14_u32.to_le_bytes().to_vec();
    body.extend(1_u32.to_le_bytes());
    body.extend(1_u32.to_le_bytes());
    body.extend([0, 0]);
    body.extend(vec![42; 1_100_000]);
    let mut codec = ZlibEncoder::new(Vec::new(), Compression::best());
    codec.write_all(&body).unwrap();
    let mut result = 1_u32.to_le_bytes().to_vec();
    result.extend(1_100_014_u32.to_le_bytes());
    result.extend(codec.finish().unwrap());
    result
}

#[test]
fn changing_file_limits_revalidates_decoded_cache_without_rereading_stored_rows() {
    for code in [2, 5, 6] {
        let mut bytes = attachment_fixture::with_payloads(&large_encoded(), EMPTY_FILE);
        bytes[20] = code;
        let forbid_children = Rc::new(Cell::new(false));
        let source = NativeSource::new(
            Watching {
                bytes: Cursor::new(bytes),
                forbid_records: Rc::new(Cell::new(false)),
                forbid_children: forbid_children.clone(),
                reads: Rc::new(Cell::new(0)),
                payload: None,
            },
            CatalogScope::new(1),
            ResourceBudget::new(64 * 1024 * 1024, 16384, 100_000_000),
            &|| false,
        )
        .unwrap()
        .with_decoded_attachments();
        let sql = "SELECT Count(Score.FileData) FROM Items";
        assert_eq!(execute(&source, sql), Err(QueryError::StructureLimit));

        forbid_children.set(true);
        let larger = AttachmentDecodeLimits::new(1_100_000).unwrap();
        let source = source.with_decoded_attachment_limits(larger);
        assert_eq!(execute(&source, sql).unwrap(), [vec![Value::Int64(2)]]);
        let source = source.with_decoded_attachment_limits(AttachmentDecodeLimits::default());
        assert_eq!(execute(&source, sql), Err(QueryError::StructureLimit));
        assert_eq!(
            execute(&source, "SELECT Count(Score.FileName) FROM Items").unwrap(),
            [vec![Value::Int64(2)]]
        );
        let source = source.with_decoded_attachment_limits(larger);
        assert_eq!(execute(&source, sql).unwrap(), [vec![Value::Int64(2)]]);
    }
}

#[test]
fn enabling_decoded_views_after_metadata_inventory_rebuilds_member_schema() {
    let source = NativeSource::new(
        Cursor::new(attachment_fixture::with_payloads(
            attachment_fixture::PAYLOAD,
            EMPTY_FILE,
        )),
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_collections();
    assert_eq!(
        execute(&source, "SELECT Count(Score.FileName) FROM Items").unwrap(),
        [vec![Value::Int64(2)]]
    );
    let source = source.with_decoded_attachment_limits(AttachmentDecodeLimits::default());
    assert_eq!(
        execute(&source, "SELECT Count(Score.FileData) FROM Items").unwrap(),
        [vec![Value::Int64(2)]]
    );
}

#[test]
fn file_policy_changes_do_not_refund_the_original_native_budget() {
    let source = NativeSource::new(
        Cursor::new(attachment_fixture::with_payloads(
            &large_encoded(),
            EMPTY_FILE,
        )),
        CatalogScope::new(1),
        ResourceBudget::new(3_500_000, 16384, 100_000_000),
        &|| false,
    )
    .unwrap()
    .with_decoded_attachment_limits(AttachmentDecodeLimits::new(1_100_000).unwrap());
    let sql = "SELECT Count(Score.FileData) FROM Items";
    assert_eq!(execute(&source, sql).unwrap(), [vec![Value::Int64(2)]]);
    let source = source.with_decoded_attachment_limits(AttachmentDecodeLimits::default());
    assert_eq!(execute(&source, sql), Err(QueryError::StructureLimit));
    let source =
        source.with_decoded_attachment_limits(AttachmentDecodeLimits::new(1_100_000).unwrap());
    assert!(
        matches!(execute(&source, sql), Err(QueryError::Limit(error)) if error.resource == Resource::Bytes)
    );
}
