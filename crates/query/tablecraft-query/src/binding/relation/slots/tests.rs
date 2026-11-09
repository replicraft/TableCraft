use crate::{
    OutputType, QueryError,
    binding::test_support::binder,
    plan::{MemberSlot, ProfiledType, Slot},
    source::TextPolicy,
};
use std::cell::Cell;
use tablecraft_core::limits::{Resource, ResourceBudget};

fn slot(name: &str, qualifier: &str, member: &str, collection: usize) -> Slot {
    Slot {
        output_type: ProfiledType::uniform(OutputType::Dynamic),
        declaration: None,
        text_policy: TextPolicy::Default,
        available: true,
        collection_index: Some(collection),
        collection_origin: None,
        retained_member: false,
        member: Some(MemberSlot::Named(member.into())),
        qualifier: Some(qualifier.into()),
        name: name.into(),
    }
}

#[test]
fn joined_namespace_pays_for_every_owned_member_name_before_publishing() {
    let left = [slot("Files", "Left", "FileName", 0)];
    let right = [slot("Other", "Right", "Caption", 0)];
    // Independent logical payload: two Slot headers, plus each written string.
    let required = (2 * size_of::<Slot>() + 5 + 4 + 8 + 5 + 5 + 7) as u64;
    let mut exact = ResourceBudget::new(required, 0, 100);
    let slots = binder(Some(&mut exact), None)
        .combine_slots(&left, &right)
        .unwrap();
    assert_eq!(exact.remaining(Resource::Bytes), 0);
    assert_eq!(slots[0].name, "Files");
    assert_eq!(slots[1].qualifier.as_deref(), Some("Right"));
    assert!(matches!(&slots[0].member, Some(MemberSlot::Named(name)) if name == "FileName"));
    assert!(matches!(&slots[1].member, Some(MemberSlot::Named(name)) if name == "Caption"));
    assert_eq!(slots[0].collection_index, Some(0));
    assert_eq!(slots[1].collection_index, Some(1));
    assert_ne!(slots[0].name.as_ptr(), left[0].name.as_ptr());

    let mut short = ResourceBudget::new(required - 1, 0, 100);
    assert!(matches!(
        binder(Some(&mut short), None).combine_slots(&left, &right),
        Err(QueryError::Limit(error)) if error.resource == Resource::Bytes
    ));
    assert_eq!(left[0].name, "Files");
    assert_eq!(right[0].qualifier.as_deref(), Some("Right"));
}

#[test]
fn successive_products_keep_the_same_cumulative_copy_allowance() {
    let left = [slot("Files", "Left", "FileName", 0)];
    let right = [slot("Other", "Right", "Caption", 0)];
    let third = [slot("Final", "Third", "Kind", 0)];
    let first_bytes = (2 * size_of::<Slot>() + 5 + 4 + 8 + 5 + 5 + 7) as u64;
    let mut work = ResourceBudget::new(first_bytes, 0, 100);
    let mut binding = binder(Some(&mut work), None);
    let first = binding.combine_slots(&left, &right).unwrap();
    assert!(matches!(
        binding.combine_slots(&first, &third),
        Err(QueryError::Limit(error)) if error.resource == Resource::Bytes
    ));
    assert_eq!(first[0].name, "Files");
    assert_eq!(first[1].collection_index, Some(1));
    assert_eq!(third[0].name, "Final");
}

#[test]
fn namespace_guards_and_copy_cancellation_preserve_original_inputs() {
    let left = [slot("Files", "Same", "FileName", 0)];
    let duplicate = [slot("Other", "same", "Caption", 0)];
    let mut empty = ResourceBudget::new(0, 0, 0);
    assert!(matches!(
        binder(Some(&mut empty), None).combine_slots(&left, &duplicate),
        Err(QueryError::AmbiguousName)
    ));

    let right = [slot("Other", "Right", "Caption", 0)];
    let calls = Cell::new(0);
    let cancelled = || {
        calls.set(calls.get() + 1);
        calls.get() >= 3
    };
    let mut work = ResourceBudget::new(10000, 0, 100);
    assert!(matches!(
        binder(Some(&mut work), Some(&cancelled)).combine_slots(&left, &right),
        Err(QueryError::Cancelled)
    ));
    assert_eq!(calls.get(), 3);
    assert_eq!(left[0].name, "Files");
    assert_eq!(right[0].name, "Other");
}
