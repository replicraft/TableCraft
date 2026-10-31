//! Ordinary CI anchors, registered only after actual export hashes are frozen.
use super::export::{digest_hex, identity};
use std::io::Write;
use tablecraft_testkit::access::canonical_before_change as model;

// Actual export hashes were frozen before registering this CI child.
const REQUESTS_SHA256: &str = "9df70b9fe37e589ddce930d08aa222adad3039cd68ff84695a50c7548a800899";
const BEFORE_SHA256: [[&str; 16]; 2] = [
    [
        "e74d8a52370760c0c91ed810f25df28914422cf5a94afbaa038bdad4049b64ea",
        "1a96700529b7d846439dc9cde126ee69045595ea5599898e0a0f5c9bf41304d1",
        "f1bc8b50e5b73ca8833eb7e5fd0480629f90658db249e299417fc75a34992b38",
        "083066a7dfa349c3e5f4af1c12bbb8782983cb3d8173b85e44cafcaeaa09620d",
        "f1bc8b50e5b73ca8833eb7e5fd0480629f90658db249e299417fc75a34992b38",
        "2fb6aec6a9877dcf1d4f967741d27be82bda014b52844b7410a48964bbd2c1d9",
        "99180dbfea995a5111782860f0f180bba9be8f35ff8b0c440a152fbcf3ad9ce6",
        "e8e81cda9522ccb0f5f21b07fcc53fe1c7afe1ca3bbd5f6bc12d5784212f929b",
        "b0aaec9a8ab427a41fe6f5f94fda0ec9e65bbd2d573ff3a718b2df8be260d0fa",
        "25474c61be6459e6302e7d50814a53787e9383b0b615395db47c3fdba10dd3e2",
        "b2793c85f30127b02c66d35cbcbeba5a17d523f46e6d5e1808e256cc44c96374",
        "0b5cb1e0f828d2190ecb333103e56172c57df9c123b9ceb4c4c26be47285337e",
        "9941c2a9e4c179986dff4d9dc98edf3ae76f1f65dda4bbc629628af1507e99e4",
        "4dfdc8b9a05bfe59cc35bc6b58b6e2f6b99907b24db95b15e46f8867df8360f8",
        "f1bc8b50e5b73ca8833eb7e5fd0480629f90658db249e299417fc75a34992b38",
        "f1bc8b50e5b73ca8833eb7e5fd0480629f90658db249e299417fc75a34992b38",
    ],
    [
        "19851ccbee133e6ec7299d7c689bc9f1598632793a34d9bdc9d7af87f92ca685",
        "c38707b5774feeeb60f0b280df0c512d51d3860cdd832977061e54154f8c11a9",
        "fcc72d56cfeb5233c04ea12e6a90d3fe69a8723714b09e09946151bc1ea5a8f2",
        "8409c8251a781cfa0d8e3a242d26229e694bdea340df4e6362594416bc001915",
        "fcc72d56cfeb5233c04ea12e6a90d3fe69a8723714b09e09946151bc1ea5a8f2",
        "e06030bc514503d49f5f4dfac58cb97ea58fabf5c63467351d10b78ba8570fdb",
        "d5ae4667039cab6a568ad0af653e887e1b086ab7b9f4b4deacbdd8b7c9f4ae71",
        "b44c3fd08eac02de31622317337c7c068cf1308f36b5beb65a1ccc0a469e8c52",
        "694bb45df2d70b4586f73958fa01ea0d334110adec95f5bcd26b6a2bca89395c",
        "deb6a41238cf73c9abc2495c8689507688dfd2ff331812dddc65e1cda1798db5",
        "afbab5b8154a21439613033a6afe34cb6040eee276030756999fa47c1b7b9949",
        "e5f1b7439cdc96a877814ba912334be3ffc27c7eca97abc3ccbe8da575bdc7d5",
        "5583123381e80ec806cebb7fa73ca78c699860f5f1aa36e6ee5fa14a786abdb3",
        "91bee458909fdba9efe4c57fa38fa116d4df0596215c77dd5ba418cdc7d0967f",
        "fcc72d56cfeb5233c04ea12e6a90d3fe69a8723714b09e09946151bc1ea5a8f2",
        "fcc72d56cfeb5233c04ea12e6a90d3fe69a8723714b09e09946151bc1ea5a8f2",
    ],
];
const EXPECTED_SHA256: [[Option<&str>; 16]; 2] = [
    [
        Some("815a2212a18bc4b5d0d4d2e85d41b07bb33b6efeb39dbfea476b3fdcbce24f0f"),
        Some("dd0822731371c622bb6a9501af38790fcda14fc7541abc78037e4816ff5da87d"),
        Some("d760b872acf9b80a88170748114e7fdbcf446d4fd23b37452ffa14065dfd5bde"),
        Some("083066a7dfa349c3e5f4af1c12bbb8782983cb3d8173b85e44cafcaeaa09620d"),
        Some("f1bc8b50e5b73ca8833eb7e5fd0480629f90658db249e299417fc75a34992b38"),
        Some("2fb6aec6a9877dcf1d4f967741d27be82bda014b52844b7410a48964bbd2c1d9"),
        Some("0ab27bd61a40b0afdc1bfa2f94390ff77d846c6792ce767d3b1d2e348a60b195"),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    ],
    [
        Some("6bfebce6559a55f72905d7e2b8ad12847c8eea83f6709efdc717b8351309f933"),
        Some("deaab531ed35b6d10aaed3a0cefc3c941c4913f27d6277c48b059d2e1d39c0fa"),
        Some("206079b441d2fb1868a7d6ec9e7c78ccd9980bf33932b013c5e074328719d47f"),
        Some("8409c8251a781cfa0d8e3a242d26229e694bdea340df4e6362594416bc001915"),
        Some("fcc72d56cfeb5233c04ea12e6a90d3fe69a8723714b09e09946151bc1ea5a8f2"),
        Some("e06030bc514503d49f5f4dfac58cb97ea58fabf5c63467351d10b78ba8570fdb"),
        Some("b2012515268f0317a320c4a8be76e2371521cdd572f4b8c1723ef1c34147dd09"),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    ],
];

fn assert_hash(bytes: &[u8], expected: &str, id: &str, role: &str) {
    assert_eq!(bytes.len(), 36_864);
    assert_digest(bytes, expected, id, role);
}

fn assert_digest(bytes: &[u8], expected: &str, id: &str, role: &str) {
    assert_eq!(
        expected.len(),
        64,
        "bind exported SHA before registering: {id} {role}"
    );
    assert_eq!(
        digest_hex(bytes),
        expected,
        "frozen normal SOURCE SHA: {id} {role}"
    );
}

#[test]
fn normal_before_change_originals_and_modeled_outcomes_retain_exported_hashes() {
    assert_eq!(model::RECIPES.len(), 16);
    let mut requests = b"header\trecipe\tcolumn\tordinal\texpected\treplacement\n".to_vec();
    let mut originals = 0;
    let mut outcomes = 0;
    for (header_index, header) in [5, 6].into_iter().enumerate() {
        for (recipe_index, recipe) in model::RECIPES.into_iter().enumerate() {
            let case = model::source(header, recipe);
            let id = identity(recipe);
            assert_hash(
                &case.before,
                BEFORE_SHA256[header_index][recipe_index],
                id,
                "original",
            );
            originals += 1;
            for update in &case.updates {
                writeln!(
                    requests,
                    "{header}\t{id}\tScore\t{}\t{}\t{}",
                    update.ordinal, update.expected, update.replacement
                )
                .unwrap();
            }
            match (&case.expected, EXPECTED_SHA256[header_index][recipe_index]) {
                (Some(bytes), Some(hash)) => {
                    assert_hash(bytes, hash, id, "modeled-after-image");
                    outcomes += 1;
                }
                (None, None) => {}
                _ => panic!("frozen modeled outcome presence changed: {id}"),
            }
        }
    }
    assert_eq!((originals, outcomes), (32, 14));
    assert_digest(&requests, REQUESTS_SHA256, "all-cases", "original-requests");
    // Literal no-op contexts remain modeled data, not normal-save admission.
    // This test never invokes a planner, program evaluator, stage or journal.
}
