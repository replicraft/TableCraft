// Pins from the independently qualified manifest; no native encoder imports.
const PINS: &[(u8, &str, &str)] = &[
    (
        5,
        "h5-before.accdb",
        "fe40ed5d67d0b82c31063b803717eaace70f36724c01951513215382c91e7ef3",
    ),
    (
        6,
        "h6-before.accdb",
        "11d395698885c7a8ef4ea01149118eedba84cdcb00030a33c4923b18abc6de21",
    ),
    (
        5,
        "h5-Int64Companions-minimum-to-zero.accdb",
        "af6215bc3bf021c1f5caa27e26b75eafa7920cf4f1d892b7ae296c6f02d52f5e",
    ),
    (
        5,
        "h5-Int64Companions-maximum-to-null.accdb",
        "1fe14ab9650c9fa6aac9f751d6e3d4b85c8f3fe46e78627d4e9ac4afd48b8967",
    ),
    (
        5,
        "h5-Int64Companions-null-to-minimum.accdb",
        "40cc2e028f63ac62e7352d9fa679a588fa54c0dbf8a939ad07204a3ede55e865",
    ),
    (
        5,
        "h5-Int64Companions-atomic-extreme-swap.accdb",
        "094e2749257df1b14d70fbae9005afbfc81054003d515481b5e16cf9e5577302",
    ),
    (
        5,
        "h5-CurrencyCompanions-minimum-to-zero.accdb",
        "1276987d73db5d2d3dd76fabe27e943e351b7381f250bcab80a0ece2cda4e603",
    ),
    (
        5,
        "h5-CurrencyCompanions-maximum-to-null.accdb",
        "8c3a859f0e02cd22abaee73e61c8af45497ba66228b2437fa9ae1d819249bec2",
    ),
    (
        5,
        "h5-CurrencyCompanions-null-to-minimum.accdb",
        "8ac5c2bbb4f03d75b38949e10f5196b27a9437a5828bf52b3a1fb8260dcb0350",
    ),
    (
        5,
        "h5-CurrencyCompanions-atomic-extreme-swap.accdb",
        "074ff489d398c9620e718d767f04492f1fba2b817ffbb99200ae8715a282966b",
    ),
    (
        6,
        "h6-Int64Companions-minimum-to-zero.accdb",
        "abeb712d57c1d5dc9730c564604bccbbcedc64a4db01ae126fa176d098737415",
    ),
    (
        6,
        "h6-Int64Companions-maximum-to-null.accdb",
        "75550d4bac6b00c6235ea6147e62b1fd381fc2e8b5396425cc0741f8384113e7",
    ),
    (
        6,
        "h6-Int64Companions-null-to-minimum.accdb",
        "3c97bffd5bc00f580147d5e4139b5449994880fd9b2c66e5399996e8129a7fef",
    ),
    (
        6,
        "h6-Int64Companions-atomic-extreme-swap.accdb",
        "2414c9996976b01dc623a9e00c8792bbb1f82304c3c33c7aaed82889dda70c8d",
    ),
    (
        6,
        "h6-CurrencyCompanions-minimum-to-zero.accdb",
        "3fd69942150f4bbcc61ad3ba3400373147f7549e45b6832fbc9e00df2532d3aa",
    ),
    (
        6,
        "h6-CurrencyCompanions-maximum-to-null.accdb",
        "d2fb55a55c3ed6f99348a8c69ad4dbacdcd3040b061bf46aef3c9b1049daffcf",
    ),
    (
        6,
        "h6-CurrencyCompanions-null-to-minimum.accdb",
        "daeabc7f57ca60a7185e51c1c69ba3e2d19228b9b5becfa569a7a8900e7c7528",
    ),
    (
        6,
        "h6-CurrencyCompanions-atomic-extreme-swap.accdb",
        "4d5ab86713209d25655d623befdfb8c5f7b80022763df0163f7538a4e802ff2b",
    ),
];

/// Verify all eighteen immutable images before returning any caller case.
pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_WIDE_SIBLING_AFTER_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let images: Vec<_> = PINS
        .iter()
        .map(|&(header, file, hash)| {
            let raw = fs::read(directory.join(file)).unwrap();
            assert_eq!(raw.len(), if header == 5 { 421888 } else { 495616 });
            verify(&raw, hash);
            (header, file, raw)
        })
        .collect();
    let mut cases = Vec::new();
    for header in [5, 6] {
        let original = format!("h{header}-before.accdb");
        for table in ["Int64Companions", "CurrencyCompanions"] {
            for (recipe, name) in [
                (Recipe::Zero, "minimum-to-zero"),
                (Recipe::Null, "maximum-to-null"),
                (Recipe::Present, "null-to-minimum"),
                (Recipe::Swap, "atomic-extreme-swap"),
                (Recipe::Noop, "before"),
                (Recipe::Empty, "before"),
            ] {
                let after_name = if name == "before" {
                    original.clone()
                } else {
                    format!("h{header}-{table}-{name}.accdb")
                };
                let view = |name: &str| {
                    images
                        .iter()
                        .find(|item| item.0 == header && item.1 == name)
                        .unwrap()
                        .2
                        .clone()
                };
                let (definition, data, roots) = match (header, table) {
                    (5, "Int64Companions") => (89, 94, [91, 92, 93]),
                    (5, "CurrencyCompanions") => (96, 101, [98, 99, 100]),
                    (6, "Int64Companions") => (106, 111, [108, 109, 110]),
                    (6, "CurrencyCompanions") => (113, 119, [115, 116, 117]),
                    _ => unreachable!(),
                };
                let case = Case {
                    header,
                    table,
                    recipe,
                    before: view(&original),
                    after: view(&after_name),
                    definition,
                    data,
                    roots,
                };
                case.assert_preserved();
                cases.push(case);
            }
        }
    }
    assert_eq!(cases.len(), 24);
    cases
}
