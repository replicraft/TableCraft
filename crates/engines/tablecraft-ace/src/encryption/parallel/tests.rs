#[path = "../../../tests/support/agile.rs"]
mod fixture;

use super::*;
use rayon::ThreadPoolBuilder;
use std::{
    fs,
    io::Cursor,
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
};

const PASSWORD: &str = "public password";

fn plain() -> Vec<u8> {
    fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap()
}

fn image() -> Vec<u8> {
    fixture::encrypted_image(&plain(), 5, PASSWORD)
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 10_000_000)
}

fn remove() -> EncryptionRewrite<'static> {
    EncryptionRewrite::Remove {
        password: Some(PASSWORD),
    }
}

#[test]
fn parallel_removal_matches_entire_serial_candidate_and_preserves_source() {
    let encrypted = image();
    let serial = crate::rewrite_encryption(
        &mut Cursor::new(&encrypted),
        remove(),
        &mut budget(),
        &|| false,
    )
    .unwrap();

    for threads in [2, 4, 8] {
        let pool = ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        let mut source = Cursor::new(encrypted.clone());
        let parallel =
            rewrite_encryption_parallel(&mut source, remove(), &pool, &mut budget(), &|| false)
                .unwrap();
        assert_eq!(parallel.as_bytes(), serial.as_bytes());
        assert_eq!(source.into_inner(), encrypted);
    }
}

#[test]
fn bad_credentials_and_cancellation_never_return_a_partial_candidate() {
    let encrypted = image();
    let pool = ThreadPoolBuilder::new().num_threads(4).build().unwrap();
    let bad = rewrite_encryption_parallel(
        &mut Cursor::new(&encrypted),
        EncryptionRewrite::Remove {
            password: Some("wrong"),
        },
        &pool,
        &mut budget(),
        &|| false,
    );
    assert!(matches!(bad, Err(InspectError::IncorrectPassword)));

    let calls = AtomicUsize::new(0);
    rewrite_encryption_parallel(
        &mut Cursor::new(&encrypted),
        remove(),
        &pool,
        &mut budget(),
        &|| {
            calls.fetch_add(1, Ordering::Relaxed);
            false
        },
    )
    .unwrap();
    let final_check = calls.load(Ordering::Relaxed);
    assert!(final_check > 2);
    for boundary in [1, final_check / 2, final_check] {
        calls.store(0, Ordering::Relaxed);
        let mut source = Cursor::new(encrypted.clone());
        let result =
            rewrite_encryption_parallel(&mut source, remove(), &pool, &mut budget(), &|| {
                calls.fetch_add(1, Ordering::Relaxed) + 1 == boundary
            });
        assert!(
            matches!(result, Err(InspectError::Cancelled)),
            "boundary {boundary}; successful total {final_check}; observed {:?}",
            result.as_ref().err()
        );
        assert_eq!(source.into_inner(), encrypted);
    }
}

#[test]
fn coordinator_bytes_and_steps_are_admitted_before_workers_touch_pages() {
    let encrypted = image();
    let decoder =
        super::super::read_agile_encryption(&mut Cursor::new(&encrypted), &mut budget(), &|| false)
            .unwrap()
            .unlock(Some(PASSWORD), &mut budget(), &|| false)
            .unwrap();
    let pool = ThreadPoolBuilder::new().num_threads(4).build().unwrap();
    let page_count = encrypted.len() / PAGE_BYTES - 1;
    let needed_bytes = page_count as u64 * (PAGE_BYTES + size_of::<PageResult>()) as u64;
    let needed_steps = 1 + page_count as u64 * PAGE_STEPS;

    for (bytes, steps) in [
        (needed_bytes - 1, needed_steps),
        (needed_bytes, needed_steps - 1),
    ] {
        let mut candidate = encrypted.clone();
        let calls = AtomicUsize::new(0);
        let result = decode_pages(
            &decoder,
            &mut candidate,
            &pool,
            &mut ResourceBudget::new(bytes, 0, steps),
            &|| {
                calls.fetch_add(1, Ordering::Relaxed);
                false
            },
        );
        assert!(matches!(result, Err(InspectError::Limit(_))));
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert_eq!(candidate, encrypted);
    }
}

#[test]
fn unsupported_operations_families_and_pool_widths_refuse_without_retry() {
    let pool = ThreadPoolBuilder::new().num_threads(2).build().unwrap();
    for operation in [
        EncryptionRewrite::Add {
            password: None,
            new_password: PASSWORD,
        },
        EncryptionRewrite::ChangePassword {
            password: Some(PASSWORD),
            new_password: "new",
        },
    ] {
        let mut source = Cursor::new(Vec::<u8>::new());
        let result =
            rewrite_encryption_parallel(&mut source, operation, &pool, &mut budget(), &|| false);
        assert!(matches!(result, Err(InspectError::UnsupportedEncryption)));
        assert_eq!(source.position(), 0);
    }

    let mut source = Cursor::new(plain());
    let result =
        rewrite_encryption_parallel(&mut source, remove(), &pool, &mut budget(), &|| false);
    assert!(matches!(result, Err(InspectError::UnsupportedEncryption)));

    let mut jet = plain();
    jet[4..20].copy_from_slice(b"Standard Jet DB\0");
    jet[20..22].copy_from_slice(&[1, 0]);
    jet[62..66].copy_from_slice(&[1, 2, 3, 4]);
    let mut source = Cursor::new(jet);
    assert!(matches!(
        inspect_encryption(&mut source, &mut budget(), &|| false),
        Ok(EncryptionInspection::JetEncoding)
    ));
    let result =
        rewrite_encryption_parallel(&mut source, remove(), &pool, &mut budget(), &|| false);
    assert!(matches!(result, Err(InspectError::UnsupportedEncryption)));

    for threads in [1, 17] {
        let pool = ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        let mut source = Cursor::new(Vec::<u8>::new());
        let result =
            rewrite_encryption_parallel(&mut source, remove(), &pool, &mut budget(), &|| false);
        assert!(matches!(result, Err(InspectError::StructureLimit)));
        assert_eq!(source.position(), 0);
    }
}
