//! Existing independently generated ACE16 scale inputs and original recipe models.
use std::{fs, path::PathBuf};
use tablecraft_core::values::Value;

pub struct Sources {
    pub collections: Vec<u8>,
    pub stored: Vec<u8>,
}

/// Load only the two registered ACE16 originals. All pins precede API consumers.
/// The caller supplies digest observation; no native codec or generator runs.
pub fn load(mut verify: impl FnMut(&[u8], &str)) -> Sources {
    let root = PathBuf::from(
        std::env::var_os("TABLECRAFT_COLLECTION_SCALE_DIR").expect("registered scale fixture root"),
    );
    let collections = fs::read(root.join("large-collections/collections-V2016.accdb")).unwrap();
    let stored = fs::read(root.join("large-stored-objects/stored-V2016.accdb")).unwrap();
    assert_eq!(collections.len(), 4_612_096);
    assert_eq!(stored.len(), 5_115_904);
    verify(
        &collections,
        "3024aeb7ffd25831245d8ab3407bf03c80ad1fd367d6bd35ae09cb60415fd459",
    );
    verify(
        &stored,
        "49877cf2cd236cb602dc1eaf2598d0fac81f0caeced02a137f3f48bb3db8d60c",
    );
    Sources {
        collections,
        stored,
    }
}

pub fn many_parent_values(parent: usize) -> Vec<Value> {
    assert!(parent < 20_000);
    let identity = i32::try_from(parent + 1).unwrap();
    if identity % 5 == 0 {
        Vec::new()
    } else {
        vec![Value::Int32(identity), Value::Null, Value::Int32(identity)]
    }
}

pub fn dense_values() -> Vec<Value> {
    (0..100)
        .map(|child| {
            if child % 17 == 0 {
                Value::Null
            } else {
                Value::Int32(child % 7)
            }
        })
        .collect()
}

pub fn timestamp(child: usize) -> u64 {
    assert!(child < 100);
    (45_292.5 + (child % 7) as f64).to_bits()
}

pub fn history_value(parent: usize, child: usize) -> Option<String> {
    assert!(parent < 200 && child < 100);
    if child.is_multiple_of(13) {
        None
    } else {
        Some(format!("v:{}:{}:é😀", parent + 1, child % 7))
    }
}

pub fn history_timestamp(child: usize) -> Option<u64> {
    assert!(child < 100);
    if child.is_multiple_of(11) {
        None
    } else {
        Some(timestamp(child))
    }
}
