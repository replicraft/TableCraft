//! Symbolic layout controls only. These ranges do not claim DATA placement or
//! manufacture complete candidates; public tests own real records and ROWs.
use super::*;
use crate::table::indexes::mutation::profile::SingleProfile;

fn layout_for(additions: &[i32], source_prefix: bool) -> layout::Layout {
    let mut before = super::routing::source(6, true);
    if source_prefix {
        let start = 15 * 4096;
        let mut seed = before[start..start + 4096].to_vec();
        seed[24..26].copy_from_slice(&4_u16.to_le_bytes());
        let keys = [
            KeyRef {
                key: Some(10),
                row: 12 << 8,
            },
            KeyRef {
                key: Some(20),
                row: (12 << 8) | 1,
            },
        ];
        let framed = super::routing::leaf(&seed, &keys);
        before[start..start + 4096].copy_from_slice(&framed);
    }
    super::routing::checked(&before);
    let image = super::routing::capture(&before);
    let profile = SingleProfile {
        column: 1,
        kind: NumericKey::new(ScalarType::Int32).unwrap(),
        descending: false,
    };
    let mut keys = [10, 20, 30]
        .into_iter()
        .enumerate()
        .map(|(slot, value)| KeyRef {
            key: Some(value),
            row: (12 << 8) | slot as u32,
        })
        .collect::<Vec<_>>();
    keys.extend(
        additions
            .iter()
            .enumerate()
            .map(|(position, value)| KeyRef {
                key: Some(i64::from(*value)),
                row: ((20 + position as u32 / 250) << 8) | (position as u32 % 250),
            }),
    );
    keys.sort_by_key(|key| (key.key, key.row));
    let boundary = keys.partition_point(|key| key.key <= Some(20));
    let ranges = [0..boundary, boundary..keys.len()];
    let mut source = image.reader();
    let mut allowance = budget();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget: &mut allowance,
        cancelled: &|| false,
        reads: 0,
    };
    let mut original = Vec::new();
    let tree =
        Tree::read::<_, ENTRY_BUFFER>(&image, &mut reader, 10, 13, &profile, 0, &mut original)
            .unwrap();
    layout::prepare(&image, &tree, &ranges, &keys, &profile, &mut reader).unwrap()
}

#[test]
fn repeated_divisions_use_first_split_then_stable_right_left_passes() {
    let layout = layout_for(&(-1500..0).collect::<Vec<_>>(), true);
    assert_eq!(layout.leaves.len(), 5);
    assert_eq!(layout.original.len(), 2);
    assert_eq!(layout.fresh.len(), 3);
    // First LEFT survives as a Fresh RIGHT when it is split again. Its symbol
    // retains seed zero, and all three created symbols remain final leaves.
    let fresh_ranges = layout
        .fresh
        .iter()
        .map(|&index| {
            assert!(matches!(layout.leaves[index].origin, layout::Origin::Fresh));
            layout.leaves[index].range.clone()
        })
        .collect::<Vec<_>>();
    assert_eq!(fresh_ranges, [375..751, 751..1126, 0..375]);
    let ordered_ranges = layout
        .order
        .iter()
        .map(|&index| layout.leaves[index].range.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        ordered_ranges,
        [0..375, 375..751, 751..1126, 1126..1502, 1502..1503]
    );
    assert!(matches!(
        layout.leaves[layout.original[0]].origin,
        layout::Origin::Existing(15)
    ));
    assert_eq!(layout.leaves[layout.original[0]].range, 1126..1502);
    let mut inventory = layout.order.clone();
    inventory.sort_unstable();
    assert_eq!(inventory, (0..layout.leaves.len()).collect::<Vec<_>>());
}

#[test]
fn separate_original_offenders_finish_in_original_child_creation_order() {
    let mut additions = (-900..0).collect::<Vec<_>>();
    additions.extend((0..900).map(|position| 1000 + position * 65536));
    let layout = layout_for(&additions, false);
    assert_eq!(layout.fresh.len(), 2);
    let first = layout.fresh[0];
    let second = layout.fresh[1];
    assert_eq!(layout.leaves[first].range, 0..451);
    assert_eq!(layout.leaves[second].range, 902..1352);
    assert_eq!(
        layout.order,
        [first, layout.original[0], second, layout.original[1]]
    );
    assert_eq!(layout.leaves[layout.original[0]].range, 451..902);
    assert_eq!(layout.leaves[layout.original[1]].range, 1352..1803);
}
