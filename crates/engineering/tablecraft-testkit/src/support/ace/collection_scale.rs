//! Typed observations shared by engine and actual browser scale consumers.
use crate::fixtures::access::collection_scale as model;
use tablecraft_ace::{
    AttachmentDecoder, ParentAttachments, ParentMultivalue, ParentVersionHistory,
};

pub fn assert_many_parent(parent: &ParentMultivalue, ordinal: usize) {
    assert_eq!(parent.values(), model::many_parent_values(ordinal));
}

pub fn assert_dense_parent(parent: &ParentMultivalue) {
    assert_eq!(parent.values(), model::dense_values());
}

pub fn assert_files(parent: &ParentAttachments, ordinal: usize) {
    assert!(ordinal <= 200);
    let expected = if ordinal < 200 { 100 } else { 0 };
    assert_eq!(parent.attachments().len(), expected);
    for (child, file) in parent.attachments().enumerate() {
        let file = file.unwrap();
        assert_eq!(file.file_url(), Some(""));
        assert_eq!(
            file.file_name(),
            Some(format!("f{}-{child}.bin", ordinal + 1).as_str())
        );
        assert_eq!(file.file_type(), Some("bin"));
        assert_eq!(file.file_flags(), Some((child % 3) as i32));
        assert_eq!(
            file.file_timestamp().map(|time| time.to_bits()),
            Some(model::timestamp(child))
        );
        assert!(
            file.encoded_file_data()
                .is_some_and(|bytes| !bytes.is_empty())
        );
    }
}

/// Explicit decoding uses the actual supplied session, never an automatic read.
pub fn assert_decoded_files(
    parent: &ParentAttachments,
    ordinal: usize,
    decoder: &mut AttachmentDecoder<'_>,
) {
    for (child, file) in parent.attachments().enumerate() {
        let decoded = file.unwrap().decode_with(decoder).unwrap().unwrap();
        assert_eq!(decoded.type_label(), "bin");
        assert_eq!(
            decoded.data(),
            [u8::try_from(ordinal + 1).unwrap(), child as u8, 255]
        );
    }
}

pub fn assert_history(parent: &ParentVersionHistory, ordinal: usize) {
    assert!(ordinal <= 200);
    let expected = if ordinal < 200 { 100 } else { 0 };
    assert_eq!(parent.stored_versions().len(), expected);
    for (child, version) in parent.stored_versions().enumerate() {
        let version = version.unwrap();
        assert_eq!(
            version.value(),
            model::history_value(ordinal, child).as_deref()
        );
        assert_eq!(
            version.modified().map(|time| time.to_bits()),
            model::history_timestamp(child)
        );
    }
}
