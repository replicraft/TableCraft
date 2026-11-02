// Literal source pins emitted after independent qualification; no native writer output.
fn all(verify: &mut impl FnMut(&[u8], &str)) -> Vec<Case> {
    vec![
        load(
            (
                "Int64Keys",
                "present-noop",
                "h5-integral-before.accdb",
                "e7538a1a69ec23953fa7f00727c277617d85848a047ccd0a2b759e618c87e5a1",
                "h5-integral-before.accdb",
                "e7538a1a69ec23953fa7f00727c277617d85848a047ccd0a2b759e618c87e5a1",
                "54434e49303030347f000000040000000070060000000000394edbdfbf5cfa5de4ab53938d5fd32f7e4caab86e508201faa1d76a0e95892009000000010000000100000000000000496e7436344b6579730800000001000000000000004b657956616c75650000000000000000030000000000000080030000000000000080",
                0,
            ),
            vec![(
                "KeyValue",
                vec![Assignment {
                    row: 0,
                    expected: Value::Int64(-9223372036854775808_i64),
                    replacement: Value::Int64(-9223372036854775808_i64),
                }],
            )],
            Layout {
                definition: 89,
                data: 94,
                roots: [91, 92, 93],
                record_length: 19,
            },
            verify,
        ),
        load(
            (
                "Int64Keys",
                "new-zero-family",
                "h5-integral-before.accdb",
                "e7538a1a69ec23953fa7f00727c277617d85848a047ccd0a2b759e618c87e5a1",
                "h5-Int64Keys-new-zero-family.accdb",
                "099d346f333309c24e906b4c7ea647990269aa3201d25c7920641d2f79baab6c",
                "54434e49303030347f000000040000000070060000000000394edbdfbf5cfa5de4ab53938d5fd32f7e4caab86e508201faa1d76a0e95892009000000010000000100000000000000496e7436344b6579730800000001000000000000004b657956616c75650000000000000000030000000000000080030000000000000000",
                1,
            ),
            vec![(
                "KeyValue",
                vec![Assignment {
                    row: 0,
                    expected: Value::Int64(-9223372036854775808_i64),
                    replacement: Value::Int64(0_i64),
                }],
            )],
            Layout {
                definition: 89,
                data: 94,
                roots: [91, 92, 93],
                record_length: 19,
            },
            verify,
        ),
        load(
            (
                "Int64Keys",
                "null-to-zero",
                "h5-integral-before.accdb",
                "e7538a1a69ec23953fa7f00727c277617d85848a047ccd0a2b759e618c87e5a1",
                "h5-Int64Keys-null-to-zero.accdb",
                "e5006707ea92e73a343567c5eec1368ba0004cc0761b853770e076999cbfc9f6",
                "54434e49303030347f000000040000000070060000000000394edbdfbf5cfa5de4ab53938d5fd32f7e4caab86e508201faa1d76a0e95892009000000010000000100000000000000496e7436344b6579730800000001000000000000004b657956616c75650100000000000000000000000000000000030000000000000000",
                1,
            ),
            vec![(
                "KeyValue",
                vec![Assignment {
                    row: 1,
                    expected: Value::Null,
                    replacement: Value::Int64(0_i64),
                }],
            )],
            Layout {
                definition: 89,
                data: 94,
                roots: [91, 92, 93],
                record_length: 19,
            },
            verify,
        ),
        load(
            (
                "Int64Keys",
                "present-to-null",
                "h5-integral-before.accdb",
                "e7538a1a69ec23953fa7f00727c277617d85848a047ccd0a2b759e618c87e5a1",
                "h5-Int64Keys-present-to-null.accdb",
                "24ebac21efffdd678b501155410240a935d989c9567fdb91c5a057bc65243beb",
                "54434e49303030347f000000040000000070060000000000394edbdfbf5cfa5de4ab53938d5fd32f7e4caab86e508201faa1d76a0e95892009000000010000000100000000000000496e7436344b6579730800000001000000000000004b657956616c75650000000000000000030000000000000080000000000000000000",
                1,
            ),
            vec![(
                "KeyValue",
                vec![Assignment {
                    row: 0,
                    expected: Value::Int64(-9223372036854775808_i64),
                    replacement: Value::Null,
                }],
            )],
            Layout {
                definition: 89,
                data: 94,
                roots: [91, 92, 93],
                record_length: 19,
            },
            verify,
        ),
        load(
            (
                "Int64Keys",
                "merge-maximum",
                "h5-integral-before.accdb",
                "e7538a1a69ec23953fa7f00727c277617d85848a047ccd0a2b759e618c87e5a1",
                "h5-Int64Keys-merge-maximum.accdb",
                "2c15af4f3a56c833dd416871986d91ca1dbdf584a3c6e88d2afc3fe7b4fa0fd2",
                "54434e49303030347f000000040000000070060000000000394edbdfbf5cfa5de4ab53938d5fd32f7e4caab86e508201faa1d76a0e95892009000000010000000100000000000000496e7436344b6579730800000001000000000000004b657956616c7565000000000000000003000000000000008003ffffffffffffff7f",
                1,
            ),
            vec![(
                "KeyValue",
                vec![Assignment {
                    row: 0,
                    expected: Value::Int64(-9223372036854775808_i64),
                    replacement: Value::Int64(9223372036854775807_i64),
                }],
            )],
            Layout {
                definition: 89,
                data: 94,
                roots: [91, 92, 93],
                record_length: 19,
            },
            verify,
        ),
        load(
            (
                "Int64Keys",
                "swap-extremes",
                "h5-integral-before.accdb",
                "e7538a1a69ec23953fa7f00727c277617d85848a047ccd0a2b759e618c87e5a1",
                "h5-Int64Keys-swap-extremes.accdb",
                "85322826192f631841a62f119deb0bdf8b5a4eb2fde85a355c0bbf58d3e1607e",
                "54434e493030303499000000040000000070060000000000394edbdfbf5cfa5de4ab53938d5fd32f7e4caab86e508201faa1d76a0e95892009000000010000000200000000000000496e7436344b6579730800000002000000000000004b657956616c7565000000000000000003000000000000008003ffffffffffffff7f020000000000000003ffffffffffffff7f030000000000000080",
                2,
            ),
            vec![(
                "KeyValue",
                vec![
                    Assignment {
                        row: 0,
                        expected: Value::Int64(-9223372036854775808_i64),
                        replacement: Value::Int64(9223372036854775807_i64),
                    },
                    Assignment {
                        row: 2,
                        expected: Value::Int64(9223372036854775807_i64),
                        replacement: Value::Int64(-9223372036854775808_i64),
                    },
                ],
            )],
            Layout {
                definition: 89,
                data: 94,
                roots: [91, 92, 93],
                record_length: 19,
            },
            verify,
        ),
        load(
            (
                "Int64Keys",
                "joint-marker",
                "h5-integral-before.accdb",
                "e7538a1a69ec23953fa7f00727c277617d85848a047ccd0a2b759e618c87e5a1",
                "h5-Int64Keys-joint-marker.accdb",
                "5602d2d5d7ebe30fabd6ad30ca911dfe2749d96212ef91030460d5de3d66c444",
                "54434e4930303034ab000000040000000070060000000000394edbdfbf5cfa5de4ab53938d5fd32f7e4caab86e508201faa1d76a0e95892009000000020000000200000000000000496e7436344b6579730800000001000000000000004b657956616c756500000000000000000300000000000000800300000000000000000600000001000000000000004d61726b65720000000000000000021100000000000000022a00000000000000",
                1,
            ),
            vec![
                (
                    "KeyValue",
                    vec![Assignment {
                        row: 0,
                        expected: Value::Int64(-9223372036854775808_i64),
                        replacement: Value::Int64(0_i64),
                    }],
                ),
                (
                    "Marker",
                    vec![Assignment {
                        row: 0,
                        expected: Value::Int32(17_i32),
                        replacement: Value::Int32(42_i32),
                    }],
                ),
            ],
            Layout {
                definition: 89,
                data: 94,
                roots: [91, 92, 93],
                record_length: 19,
            },
            verify,
        ),
        load(
            (
                "CurrencyKeys",
                "present-noop",
                "h5-integral-before.accdb",
                "e7538a1a69ec23953fa7f00727c277617d85848a047ccd0a2b759e618c87e5a1",
                "h5-integral-before.accdb",
                "e7538a1a69ec23953fa7f00727c277617d85848a047ccd0a2b759e618c87e5a1",
                "54434e493030303482000000040000000070060000000000394edbdfbf5cfa5de4ab53938d5fd32f7e4caab86e508201faa1d76a0e9589200c00000001000000010000000000000043757272656e63794b6579730800000001000000000000004b657956616c75650000000000000000040000000000000080040000000000000080",
                0,
            ),
            vec![(
                "KeyValue",
                vec![Assignment {
                    row: 0,
                    expected: Value::Currency(Currency::from_scaled(-9223372036854775808_i64)),
                    replacement: Value::Currency(Currency::from_scaled(-9223372036854775808_i64)),
                }],
            )],
            Layout {
                definition: 96,
                data: 101,
                roots: [98, 99, 100],
                record_length: 19,
            },
            verify,
        ),
        load(
            (
                "CurrencyKeys",
                "new-zero-family",
                "h5-integral-before.accdb",
                "e7538a1a69ec23953fa7f00727c277617d85848a047ccd0a2b759e618c87e5a1",
                "h5-CurrencyKeys-new-zero-family.accdb",
                "bdb6de2f347cfc05ea06e9a2ac126811d194594f119f537974ff19188b2ff98f",
                "54434e493030303482000000040000000070060000000000394edbdfbf5cfa5de4ab53938d5fd32f7e4caab86e508201faa1d76a0e9589200c00000001000000010000000000000043757272656e63794b6579730800000001000000000000004b657956616c75650000000000000000040000000000000080040000000000000000",
                1,
            ),
            vec![(
                "KeyValue",
                vec![Assignment {
                    row: 0,
                    expected: Value::Currency(Currency::from_scaled(-9223372036854775808_i64)),
                    replacement: Value::Currency(Currency::from_scaled(0_i64)),
                }],
            )],
            Layout {
                definition: 96,
                data: 101,
                roots: [98, 99, 100],
                record_length: 19,
            },
            verify,
        ),
        load(
            (
                "CurrencyKeys",
                "null-to-zero",
                "h5-integral-before.accdb",
                "e7538a1a69ec23953fa7f00727c277617d85848a047ccd0a2b759e618c87e5a1",
                "h5-CurrencyKeys-null-to-zero.accdb",
                "be5048a02b0da963df27564aed847b9d0bcfb9a8637aea576b514a4ebf3722bd",
                "54434e493030303482000000040000000070060000000000394edbdfbf5cfa5de4ab53938d5fd32f7e4caab86e508201faa1d76a0e9589200c00000001000000010000000000000043757272656e63794b6579730800000001000000000000004b657956616c75650100000000000000000000000000000000040000000000000000",
                1,
            ),
            vec![(
                "KeyValue",
                vec![Assignment {
                    row: 1,
                    expected: Value::Null,
                    replacement: Value::Currency(Currency::from_scaled(0_i64)),
                }],
            )],
            Layout {
                definition: 96,
                data: 101,
                roots: [98, 99, 100],
                record_length: 19,
            },
            verify,
        ),
        load(
            (
                "CurrencyKeys",
                "present-to-null",
                "h5-integral-before.accdb",
                "e7538a1a69ec23953fa7f00727c277617d85848a047ccd0a2b759e618c87e5a1",
                "h5-CurrencyKeys-present-to-null.accdb",
                "3ed5df41581ed2e0bf0072f75136b5b5ab7991613177db45cca70e64b9efee2d",
                "54434e493030303482000000040000000070060000000000394edbdfbf5cfa5de4ab53938d5fd32f7e4caab86e508201faa1d76a0e9589200c00000001000000010000000000000043757272656e63794b6579730800000001000000000000004b657956616c75650000000000000000040000000000000080000000000000000000",
                1,
            ),
            vec![(
                "KeyValue",
                vec![Assignment {
                    row: 0,
                    expected: Value::Currency(Currency::from_scaled(-9223372036854775808_i64)),
                    replacement: Value::Null,
                }],
            )],
            Layout {
                definition: 96,
                data: 101,
                roots: [98, 99, 100],
                record_length: 19,
            },
            verify,
        ),
        load(
            (
                "CurrencyKeys",
                "merge-maximum",
                "h5-integral-before.accdb",
                "e7538a1a69ec23953fa7f00727c277617d85848a047ccd0a2b759e618c87e5a1",
                "h5-CurrencyKeys-merge-maximum.accdb",
                "3453ea5cc84e64e8a9ae16a8f3245621922dc65d1f259c927b838e2e416ab4d6",
                "54434e493030303482000000040000000070060000000000394edbdfbf5cfa5de4ab53938d5fd32f7e4caab86e508201faa1d76a0e9589200c00000001000000010000000000000043757272656e63794b6579730800000001000000000000004b657956616c7565000000000000000004000000000000008004ffffffffffffff7f",
                1,
            ),
            vec![(
                "KeyValue",
                vec![Assignment {
                    row: 0,
                    expected: Value::Currency(Currency::from_scaled(-9223372036854775808_i64)),
                    replacement: Value::Currency(Currency::from_scaled(9223372036854775807_i64)),
                }],
            )],
            Layout {
                definition: 96,
                data: 101,
                roots: [98, 99, 100],
                record_length: 19,
            },
            verify,
        ),
        load(
            (
                "CurrencyKeys",
                "swap-extremes",
                "h5-integral-before.accdb",
                "e7538a1a69ec23953fa7f00727c277617d85848a047ccd0a2b759e618c87e5a1",
                "h5-CurrencyKeys-swap-extremes.accdb",
                "fcab39e9d3553a97fe69af02dddcac8d317a16ef7ea25560aab92d199dde9a8f",
                "54434e49303030349c000000040000000070060000000000394edbdfbf5cfa5de4ab53938d5fd32f7e4caab86e508201faa1d76a0e9589200c00000001000000020000000000000043757272656e63794b6579730800000002000000000000004b657956616c7565000000000000000004000000000000008004ffffffffffffff7f020000000000000004ffffffffffffff7f040000000000000080",
                2,
            ),
            vec![(
                "KeyValue",
                vec![
                    Assignment {
                        row: 0,
                        expected: Value::Currency(Currency::from_scaled(-9223372036854775808_i64)),
                        replacement: Value::Currency(Currency::from_scaled(
                            9223372036854775807_i64,
                        )),
                    },
                    Assignment {
                        row: 2,
                        expected: Value::Currency(Currency::from_scaled(9223372036854775807_i64)),
                        replacement: Value::Currency(Currency::from_scaled(
                            -9223372036854775808_i64,
                        )),
                    },
                ],
            )],
            Layout {
                definition: 96,
                data: 101,
                roots: [98, 99, 100],
                record_length: 19,
            },
            verify,
        ),
        load(
            (
                "CurrencyKeys",
                "joint-marker",
                "h5-integral-before.accdb",
                "e7538a1a69ec23953fa7f00727c277617d85848a047ccd0a2b759e618c87e5a1",
                "h5-CurrencyKeys-joint-marker.accdb",
                "7fe5ce50517fd4e079e4882146739e7d4d31431529fb2a6a39a8bbbb859e26a1",
                "54434e4930303034ae000000040000000070060000000000394edbdfbf5cfa5de4ab53938d5fd32f7e4caab86e508201faa1d76a0e9589200c00000002000000020000000000000043757272656e63794b6579730800000001000000000000004b657956616c756500000000000000000400000000000000800400000000000000000600000001000000000000004d61726b65720000000000000000021100000000000000022a00000000000000",
                1,
            ),
            vec![
                (
                    "KeyValue",
                    vec![Assignment {
                        row: 0,
                        expected: Value::Currency(Currency::from_scaled(-9223372036854775808_i64)),
                        replacement: Value::Currency(Currency::from_scaled(0_i64)),
                    }],
                ),
                (
                    "Marker",
                    vec![Assignment {
                        row: 0,
                        expected: Value::Int32(17_i32),
                        replacement: Value::Int32(42_i32),
                    }],
                ),
            ],
            Layout {
                definition: 96,
                data: 101,
                roots: [98, 99, 100],
                record_length: 19,
            },
            verify,
        ),
        load(
            (
                "Int64Keys",
                "present-noop",
                "h6-integral-before.accdb",
                "3779da0b09d0fd424e62929149578f63309b6d8f63a0ef7502e658b20ced377e",
                "h6-integral-before.accdb",
                "3779da0b09d0fd424e62929149578f63309b6d8f63a0ef7502e658b20ced377e",
                "54434e49303030347f00000004000000008007000000000060cea74b9261bc87837e655f815499c633163de254f0d60b49f8df31cd4476d209000000010000000100000000000000496e7436344b6579730800000001000000000000004b657956616c75650000000000000000030000000000000080030000000000000080",
                0,
            ),
            vec![(
                "KeyValue",
                vec![Assignment {
                    row: 0,
                    expected: Value::Int64(-9223372036854775808_i64),
                    replacement: Value::Int64(-9223372036854775808_i64),
                }],
            )],
            Layout {
                definition: 106,
                data: 111,
                roots: [108, 109, 110],
                record_length: 19,
            },
            verify,
        ),
        load(
            (
                "Int64Keys",
                "new-zero-family",
                "h6-integral-before.accdb",
                "3779da0b09d0fd424e62929149578f63309b6d8f63a0ef7502e658b20ced377e",
                "h6-Int64Keys-new-zero-family.accdb",
                "731caa96b245d0f94ea1a0b1f61dfe948f8d59c49aa71623ade9a50b829414c2",
                "54434e49303030347f00000004000000008007000000000060cea74b9261bc87837e655f815499c633163de254f0d60b49f8df31cd4476d209000000010000000100000000000000496e7436344b6579730800000001000000000000004b657956616c75650000000000000000030000000000000080030000000000000000",
                1,
            ),
            vec![(
                "KeyValue",
                vec![Assignment {
                    row: 0,
                    expected: Value::Int64(-9223372036854775808_i64),
                    replacement: Value::Int64(0_i64),
                }],
            )],
            Layout {
                definition: 106,
                data: 111,
                roots: [108, 109, 110],
                record_length: 19,
            },
            verify,
        ),
        load(
            (
                "Int64Keys",
                "null-to-zero",
                "h6-integral-before.accdb",
                "3779da0b09d0fd424e62929149578f63309b6d8f63a0ef7502e658b20ced377e",
                "h6-Int64Keys-null-to-zero.accdb",
                "7a9c676e25a4bde41dbd8a7760d56d50cd375263021714881d319c0be2d5f410",
                "54434e49303030347f00000004000000008007000000000060cea74b9261bc87837e655f815499c633163de254f0d60b49f8df31cd4476d209000000010000000100000000000000496e7436344b6579730800000001000000000000004b657956616c75650100000000000000000000000000000000030000000000000000",
                1,
            ),
            vec![(
                "KeyValue",
                vec![Assignment {
                    row: 1,
                    expected: Value::Null,
                    replacement: Value::Int64(0_i64),
                }],
            )],
            Layout {
                definition: 106,
                data: 111,
                roots: [108, 109, 110],
                record_length: 19,
            },
            verify,
        ),
        load(
            (
                "Int64Keys",
                "present-to-null",
                "h6-integral-before.accdb",
                "3779da0b09d0fd424e62929149578f63309b6d8f63a0ef7502e658b20ced377e",
                "h6-Int64Keys-present-to-null.accdb",
                "12096cf1d9cb30de7c140d8187144bb75ea077a8c80859e581e46070cf4f1259",
                "54434e49303030347f00000004000000008007000000000060cea74b9261bc87837e655f815499c633163de254f0d60b49f8df31cd4476d209000000010000000100000000000000496e7436344b6579730800000001000000000000004b657956616c75650000000000000000030000000000000080000000000000000000",
                1,
            ),
            vec![(
                "KeyValue",
                vec![Assignment {
                    row: 0,
                    expected: Value::Int64(-9223372036854775808_i64),
                    replacement: Value::Null,
                }],
            )],
            Layout {
                definition: 106,
                data: 111,
                roots: [108, 109, 110],
                record_length: 19,
            },
            verify,
        ),
        load(
            (
                "Int64Keys",
                "merge-maximum",
                "h6-integral-before.accdb",
                "3779da0b09d0fd424e62929149578f63309b6d8f63a0ef7502e658b20ced377e",
                "h6-Int64Keys-merge-maximum.accdb",
                "3ae10478cca3e83a00a72edab6c460e0bb82ec155d6bdbd9002cda5fcb311e90",
                "54434e49303030347f00000004000000008007000000000060cea74b9261bc87837e655f815499c633163de254f0d60b49f8df31cd4476d209000000010000000100000000000000496e7436344b6579730800000001000000000000004b657956616c7565000000000000000003000000000000008003ffffffffffffff7f",
                1,
            ),
            vec![(
                "KeyValue",
                vec![Assignment {
                    row: 0,
                    expected: Value::Int64(-9223372036854775808_i64),
                    replacement: Value::Int64(9223372036854775807_i64),
                }],
            )],
            Layout {
                definition: 106,
                data: 111,
                roots: [108, 109, 110],
                record_length: 19,
            },
            verify,
        ),
        load(
            (
                "Int64Keys",
                "swap-extremes",
                "h6-integral-before.accdb",
                "3779da0b09d0fd424e62929149578f63309b6d8f63a0ef7502e658b20ced377e",
                "h6-Int64Keys-swap-extremes.accdb",
                "29fe7b9c8afe1defb9fa405f778dedb1899634fa291c34d30f624f87157dec18",
                "54434e49303030349900000004000000008007000000000060cea74b9261bc87837e655f815499c633163de254f0d60b49f8df31cd4476d209000000010000000200000000000000496e7436344b6579730800000002000000000000004b657956616c7565000000000000000003000000000000008003ffffffffffffff7f020000000000000003ffffffffffffff7f030000000000000080",
                2,
            ),
            vec![(
                "KeyValue",
                vec![
                    Assignment {
                        row: 0,
                        expected: Value::Int64(-9223372036854775808_i64),
                        replacement: Value::Int64(9223372036854775807_i64),
                    },
                    Assignment {
                        row: 2,
                        expected: Value::Int64(9223372036854775807_i64),
                        replacement: Value::Int64(-9223372036854775808_i64),
                    },
                ],
            )],
            Layout {
                definition: 106,
                data: 111,
                roots: [108, 109, 110],
                record_length: 19,
            },
            verify,
        ),
        load(
            (
                "Int64Keys",
                "joint-marker",
                "h6-integral-before.accdb",
                "3779da0b09d0fd424e62929149578f63309b6d8f63a0ef7502e658b20ced377e",
                "h6-Int64Keys-joint-marker.accdb",
                "10579f9b802a6387df77867019d00b7f2d5ed907e22bb75ec93e9fd837395947",
                "54434e4930303034ab00000004000000008007000000000060cea74b9261bc87837e655f815499c633163de254f0d60b49f8df31cd4476d209000000020000000200000000000000496e7436344b6579730800000001000000000000004b657956616c756500000000000000000300000000000000800300000000000000000600000001000000000000004d61726b65720000000000000000021100000000000000022a00000000000000",
                1,
            ),
            vec![
                (
                    "KeyValue",
                    vec![Assignment {
                        row: 0,
                        expected: Value::Int64(-9223372036854775808_i64),
                        replacement: Value::Int64(0_i64),
                    }],
                ),
                (
                    "Marker",
                    vec![Assignment {
                        row: 0,
                        expected: Value::Int32(17_i32),
                        replacement: Value::Int32(42_i32),
                    }],
                ),
            ],
            Layout {
                definition: 106,
                data: 111,
                roots: [108, 109, 110],
                record_length: 19,
            },
            verify,
        ),
        load(
            (
                "CurrencyKeys",
                "present-noop",
                "h6-integral-before.accdb",
                "3779da0b09d0fd424e62929149578f63309b6d8f63a0ef7502e658b20ced377e",
                "h6-integral-before.accdb",
                "3779da0b09d0fd424e62929149578f63309b6d8f63a0ef7502e658b20ced377e",
                "54434e49303030348200000004000000008007000000000060cea74b9261bc87837e655f815499c633163de254f0d60b49f8df31cd4476d20c00000001000000010000000000000043757272656e63794b6579730800000001000000000000004b657956616c75650000000000000000040000000000000080040000000000000080",
                0,
            ),
            vec![(
                "KeyValue",
                vec![Assignment {
                    row: 0,
                    expected: Value::Currency(Currency::from_scaled(-9223372036854775808_i64)),
                    replacement: Value::Currency(Currency::from_scaled(-9223372036854775808_i64)),
                }],
            )],
            Layout {
                definition: 113,
                data: 118,
                roots: [115, 116, 117],
                record_length: 19,
            },
            verify,
        ),
        load(
            (
                "CurrencyKeys",
                "new-zero-family",
                "h6-integral-before.accdb",
                "3779da0b09d0fd424e62929149578f63309b6d8f63a0ef7502e658b20ced377e",
                "h6-CurrencyKeys-new-zero-family.accdb",
                "b5048623acd6e515caf871a1334ce142089f172bae8aafac40378be2b9880511",
                "54434e49303030348200000004000000008007000000000060cea74b9261bc87837e655f815499c633163de254f0d60b49f8df31cd4476d20c00000001000000010000000000000043757272656e63794b6579730800000001000000000000004b657956616c75650000000000000000040000000000000080040000000000000000",
                1,
            ),
            vec![(
                "KeyValue",
                vec![Assignment {
                    row: 0,
                    expected: Value::Currency(Currency::from_scaled(-9223372036854775808_i64)),
                    replacement: Value::Currency(Currency::from_scaled(0_i64)),
                }],
            )],
            Layout {
                definition: 113,
                data: 118,
                roots: [115, 116, 117],
                record_length: 19,
            },
            verify,
        ),
        load(
            (
                "CurrencyKeys",
                "null-to-zero",
                "h6-integral-before.accdb",
                "3779da0b09d0fd424e62929149578f63309b6d8f63a0ef7502e658b20ced377e",
                "h6-CurrencyKeys-null-to-zero.accdb",
                "ab0710e560e3158457c45e8c3f33a86d76dc2ceb6b4eb28f2ae0088eb08b154b",
                "54434e49303030348200000004000000008007000000000060cea74b9261bc87837e655f815499c633163de254f0d60b49f8df31cd4476d20c00000001000000010000000000000043757272656e63794b6579730800000001000000000000004b657956616c75650100000000000000000000000000000000040000000000000000",
                1,
            ),
            vec![(
                "KeyValue",
                vec![Assignment {
                    row: 1,
                    expected: Value::Null,
                    replacement: Value::Currency(Currency::from_scaled(0_i64)),
                }],
            )],
            Layout {
                definition: 113,
                data: 118,
                roots: [115, 116, 117],
                record_length: 19,
            },
            verify,
        ),
        load(
            (
                "CurrencyKeys",
                "present-to-null",
                "h6-integral-before.accdb",
                "3779da0b09d0fd424e62929149578f63309b6d8f63a0ef7502e658b20ced377e",
                "h6-CurrencyKeys-present-to-null.accdb",
                "e6d9bae22381ed81b87a2aadf2226575edb3fb057deb184a2da62d70a93eacf8",
                "54434e49303030348200000004000000008007000000000060cea74b9261bc87837e655f815499c633163de254f0d60b49f8df31cd4476d20c00000001000000010000000000000043757272656e63794b6579730800000001000000000000004b657956616c75650000000000000000040000000000000080000000000000000000",
                1,
            ),
            vec![(
                "KeyValue",
                vec![Assignment {
                    row: 0,
                    expected: Value::Currency(Currency::from_scaled(-9223372036854775808_i64)),
                    replacement: Value::Null,
                }],
            )],
            Layout {
                definition: 113,
                data: 118,
                roots: [115, 116, 117],
                record_length: 19,
            },
            verify,
        ),
        load(
            (
                "CurrencyKeys",
                "merge-maximum",
                "h6-integral-before.accdb",
                "3779da0b09d0fd424e62929149578f63309b6d8f63a0ef7502e658b20ced377e",
                "h6-CurrencyKeys-merge-maximum.accdb",
                "920bf82a41272b72ba3eca97a5536b1f8e40e7c0d1dd183551b960ecfbedeaaf",
                "54434e49303030348200000004000000008007000000000060cea74b9261bc87837e655f815499c633163de254f0d60b49f8df31cd4476d20c00000001000000010000000000000043757272656e63794b6579730800000001000000000000004b657956616c7565000000000000000004000000000000008004ffffffffffffff7f",
                1,
            ),
            vec![(
                "KeyValue",
                vec![Assignment {
                    row: 0,
                    expected: Value::Currency(Currency::from_scaled(-9223372036854775808_i64)),
                    replacement: Value::Currency(Currency::from_scaled(9223372036854775807_i64)),
                }],
            )],
            Layout {
                definition: 113,
                data: 118,
                roots: [115, 116, 117],
                record_length: 19,
            },
            verify,
        ),
        load(
            (
                "CurrencyKeys",
                "swap-extremes",
                "h6-integral-before.accdb",
                "3779da0b09d0fd424e62929149578f63309b6d8f63a0ef7502e658b20ced377e",
                "h6-CurrencyKeys-swap-extremes.accdb",
                "0bf2ba267fba0e9c199bd58513bf6c82ef8f600e0a3057e6663360a38c0bb89b",
                "54434e49303030349c00000004000000008007000000000060cea74b9261bc87837e655f815499c633163de254f0d60b49f8df31cd4476d20c00000001000000020000000000000043757272656e63794b6579730800000002000000000000004b657956616c7565000000000000000004000000000000008004ffffffffffffff7f020000000000000004ffffffffffffff7f040000000000000080",
                2,
            ),
            vec![(
                "KeyValue",
                vec![
                    Assignment {
                        row: 0,
                        expected: Value::Currency(Currency::from_scaled(-9223372036854775808_i64)),
                        replacement: Value::Currency(Currency::from_scaled(
                            9223372036854775807_i64,
                        )),
                    },
                    Assignment {
                        row: 2,
                        expected: Value::Currency(Currency::from_scaled(9223372036854775807_i64)),
                        replacement: Value::Currency(Currency::from_scaled(
                            -9223372036854775808_i64,
                        )),
                    },
                ],
            )],
            Layout {
                definition: 113,
                data: 118,
                roots: [115, 116, 117],
                record_length: 19,
            },
            verify,
        ),
        load(
            (
                "CurrencyKeys",
                "joint-marker",
                "h6-integral-before.accdb",
                "3779da0b09d0fd424e62929149578f63309b6d8f63a0ef7502e658b20ced377e",
                "h6-CurrencyKeys-joint-marker.accdb",
                "65567963036d3430bbcb53c7aa4f139891daf1d39ec8cd423ba0021ea3f10bf3",
                "54434e4930303034ae00000004000000008007000000000060cea74b9261bc87837e655f815499c633163de254f0d60b49f8df31cd4476d20c00000002000000020000000000000043757272656e63794b6579730800000001000000000000004b657956616c756500000000000000000400000000000000800400000000000000000600000001000000000000004d61726b65720000000000000000021100000000000000022a00000000000000",
                1,
            ),
            vec![
                (
                    "KeyValue",
                    vec![Assignment {
                        row: 0,
                        expected: Value::Currency(Currency::from_scaled(-9223372036854775808_i64)),
                        replacement: Value::Currency(Currency::from_scaled(0_i64)),
                    }],
                ),
                (
                    "Marker",
                    vec![Assignment {
                        row: 0,
                        expected: Value::Int32(17_i32),
                        replacement: Value::Int32(42_i32),
                    }],
                ),
            ],
            Layout {
                definition: 113,
                data: 118,
                roots: [115, 116, 117],
                record_length: 19,
            },
            verify,
        ),
        load(
            (
                "Int64WidePrimary",
                "null-to-zero",
                "h5-wide-primary-before.accdb",
                "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
                "h5-Int64WidePrimary-null-to-zero.accdb",
                "38b9dbbfba3d566cd521c16ae7080eeff62dbbc7be68f565c35e376bb80089f4",
                "54434e49303030348600000004000000004008000000000020978751b249aece8e384dedad2a3240a96f6e13135d8873676a25b1c0f6825510000000010000000100000000000000496e743634576964655072696d6172790800000001000000000000004b657956616c75650100000000000000000000000000000000030000000000000000",
                1,
            ),
            vec![(
                "KeyValue",
                vec![Assignment {
                    row: 1,
                    expected: Value::Null,
                    replacement: Value::Int64(0_i64),
                }],
            )],
            Layout {
                definition: 103,
                data: 108,
                roots: [105, 106, 107],
                record_length: 23,
            },
            verify,
        ),
        load(
            (
                "Int64WidePrimary",
                "present-to-null",
                "h5-wide-primary-before.accdb",
                "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
                "h5-Int64WidePrimary-present-to-null.accdb",
                "6361b5923fbd3355393401611ab1398ba2567d4fd9c02e0f5b7f2bad01e83d1d",
                "54434e49303030348600000004000000004008000000000020978751b249aece8e384dedad2a3240a96f6e13135d8873676a25b1c0f6825510000000010000000100000000000000496e743634576964655072696d6172790800000001000000000000004b657956616c75650000000000000000030000000000000080000000000000000000",
                1,
            ),
            vec![(
                "KeyValue",
                vec![Assignment {
                    row: 0,
                    expected: Value::Int64(-9223372036854775808_i64),
                    replacement: Value::Null,
                }],
            )],
            Layout {
                definition: 103,
                data: 108,
                roots: [105, 106, 107],
                record_length: 23,
            },
            verify,
        ),
        load(
            (
                "Int64WidePrimary",
                "swap-extremes",
                "h5-wide-primary-before.accdb",
                "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
                "h5-Int64WidePrimary-swap-extremes.accdb",
                "869a4a78d4e28b54b33486d036b33de3583630933ad460f312e32fdfbdc849d7",
                "54434e4930303034a000000004000000004008000000000020978751b249aece8e384dedad2a3240a96f6e13135d8873676a25b1c0f6825510000000010000000200000000000000496e743634576964655072696d6172790800000002000000000000004b657956616c7565000000000000000003000000000000008003ffffffffffffff7f020000000000000003ffffffffffffff7f030000000000000080",
                2,
            ),
            vec![(
                "KeyValue",
                vec![
                    Assignment {
                        row: 0,
                        expected: Value::Int64(-9223372036854775808_i64),
                        replacement: Value::Int64(9223372036854775807_i64),
                    },
                    Assignment {
                        row: 2,
                        expected: Value::Int64(9223372036854775807_i64),
                        replacement: Value::Int64(-9223372036854775808_i64),
                    },
                ],
            )],
            Layout {
                definition: 103,
                data: 108,
                roots: [105, 106, 107],
                record_length: 23,
            },
            verify,
        ),
        load(
            (
                "Int64WidePrimary",
                "primary-new-minus-one",
                "h5-wide-primary-before.accdb",
                "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
                "h5-Int64WidePrimary-primary-new-minus-one.accdb",
                "0dfc7fff5e30af0bac4c92a037312aea5f353fa3fb62de0d1226f5c7ba4d14e5",
                "54434e49303030348000000004000000004008000000000020978751b249aece8e384dedad2a3240a96f6e13135d8873676a25b1c0f6825510000000010000000100000000000000496e743634576964655072696d6172790200000001000000000000004964000000000000000003000000000000008003ffffffffffffffff",
                1,
            ),
            vec![(
                "Id",
                vec![Assignment {
                    row: 0,
                    expected: Value::Int64(-9223372036854775808_i64),
                    replacement: Value::Int64(-1_i64),
                }],
            )],
            Layout {
                definition: 103,
                data: 108,
                roots: [105, 106, 107],
                record_length: 23,
            },
            verify,
        ),
        load(
            (
                "Int64WidePrimary",
                "primary-swap-extremes",
                "h5-wide-primary-before.accdb",
                "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
                "h5-Int64WidePrimary-primary-swap-extremes.accdb",
                "ab7e40e4d9bac3b7d25ae1e73f34f7d3462aadbe956c5c94dd8cb3f2959370fb",
                "54434e49303030349a00000004000000004008000000000020978751b249aece8e384dedad2a3240a96f6e13135d8873676a25b1c0f6825510000000010000000200000000000000496e743634576964655072696d6172790200000002000000000000004964000000000000000003000000000000008003ffffffffffffff7f020000000000000003ffffffffffffff7f030000000000000080",
                2,
            ),
            vec![(
                "Id",
                vec![
                    Assignment {
                        row: 0,
                        expected: Value::Int64(-9223372036854775808_i64),
                        replacement: Value::Int64(9223372036854775807_i64),
                    },
                    Assignment {
                        row: 2,
                        expected: Value::Int64(9223372036854775807_i64),
                        replacement: Value::Int64(-9223372036854775808_i64),
                    },
                ],
            )],
            Layout {
                definition: 103,
                data: 108,
                roots: [105, 106, 107],
                record_length: 23,
            },
            verify,
        ),
        load(
            (
                "CurrencyWidePrimary",
                "null-to-zero",
                "h5-wide-primary-before.accdb",
                "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
                "h5-CurrencyWidePrimary-null-to-zero.accdb",
                "fd4ac870eb564d08509f7a4561448765c099fd24fffe50969f20083b9248f80b",
                "54434e49303030348900000004000000004008000000000020978751b249aece8e384dedad2a3240a96f6e13135d8873676a25b1c0f682551300000001000000010000000000000043757272656e6379576964655072696d6172790800000001000000000000004b657956616c75650100000000000000000000000000000000040000000000000000",
                1,
            ),
            vec![(
                "KeyValue",
                vec![Assignment {
                    row: 1,
                    expected: Value::Null,
                    replacement: Value::Currency(Currency::from_scaled(0_i64)),
                }],
            )],
            Layout {
                definition: 110,
                data: 115,
                roots: [112, 113, 114],
                record_length: 23,
            },
            verify,
        ),
        load(
            (
                "CurrencyWidePrimary",
                "present-to-null",
                "h5-wide-primary-before.accdb",
                "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
                "h5-CurrencyWidePrimary-present-to-null.accdb",
                "265d8f96c22e6512416afc355cae64772adc233520af5dfeea7fd9cc80645937",
                "54434e49303030348900000004000000004008000000000020978751b249aece8e384dedad2a3240a96f6e13135d8873676a25b1c0f682551300000001000000010000000000000043757272656e6379576964655072696d6172790800000001000000000000004b657956616c75650000000000000000040000000000000080000000000000000000",
                1,
            ),
            vec![(
                "KeyValue",
                vec![Assignment {
                    row: 0,
                    expected: Value::Currency(Currency::from_scaled(-9223372036854775808_i64)),
                    replacement: Value::Null,
                }],
            )],
            Layout {
                definition: 110,
                data: 115,
                roots: [112, 113, 114],
                record_length: 23,
            },
            verify,
        ),
        load(
            (
                "CurrencyWidePrimary",
                "swap-extremes",
                "h5-wide-primary-before.accdb",
                "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
                "h5-CurrencyWidePrimary-swap-extremes.accdb",
                "6959685aca206898596cddb77a64619085ff51f35f505bf94ff94c9f2e6f1b00",
                "54434e4930303034a300000004000000004008000000000020978751b249aece8e384dedad2a3240a96f6e13135d8873676a25b1c0f682551300000001000000020000000000000043757272656e6379576964655072696d6172790800000002000000000000004b657956616c7565000000000000000004000000000000008004ffffffffffffff7f020000000000000004ffffffffffffff7f040000000000000080",
                2,
            ),
            vec![(
                "KeyValue",
                vec![
                    Assignment {
                        row: 0,
                        expected: Value::Currency(Currency::from_scaled(-9223372036854775808_i64)),
                        replacement: Value::Currency(Currency::from_scaled(
                            9223372036854775807_i64,
                        )),
                    },
                    Assignment {
                        row: 2,
                        expected: Value::Currency(Currency::from_scaled(9223372036854775807_i64)),
                        replacement: Value::Currency(Currency::from_scaled(
                            -9223372036854775808_i64,
                        )),
                    },
                ],
            )],
            Layout {
                definition: 110,
                data: 115,
                roots: [112, 113, 114],
                record_length: 23,
            },
            verify,
        ),
        load(
            (
                "CurrencyWidePrimary",
                "primary-new-minus-one",
                "h5-wide-primary-before.accdb",
                "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
                "h5-CurrencyWidePrimary-primary-new-minus-one.accdb",
                "7507108b700cdad1246dff88827729c93f3d214e7622d80a3f1d872c5715144e",
                "54434e49303030348300000004000000004008000000000020978751b249aece8e384dedad2a3240a96f6e13135d8873676a25b1c0f682551300000001000000010000000000000043757272656e6379576964655072696d6172790200000001000000000000004964000000000000000004000000000000008004ffffffffffffffff",
                1,
            ),
            vec![(
                "Id",
                vec![Assignment {
                    row: 0,
                    expected: Value::Currency(Currency::from_scaled(-9223372036854775808_i64)),
                    replacement: Value::Currency(Currency::from_scaled(-1_i64)),
                }],
            )],
            Layout {
                definition: 110,
                data: 115,
                roots: [112, 113, 114],
                record_length: 23,
            },
            verify,
        ),
        load(
            (
                "CurrencyWidePrimary",
                "primary-swap-extremes",
                "h5-wide-primary-before.accdb",
                "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
                "h5-CurrencyWidePrimary-primary-swap-extremes.accdb",
                "e55308f475f3b6d7b9086af1abe28a52dee663f8e0cd281dfb42e6e92edcf38b",
                "54434e49303030349d00000004000000004008000000000020978751b249aece8e384dedad2a3240a96f6e13135d8873676a25b1c0f682551300000001000000020000000000000043757272656e6379576964655072696d6172790200000002000000000000004964000000000000000004000000000000008004ffffffffffffff7f020000000000000004ffffffffffffff7f040000000000000080",
                2,
            ),
            vec![(
                "Id",
                vec![
                    Assignment {
                        row: 0,
                        expected: Value::Currency(Currency::from_scaled(-9223372036854775808_i64)),
                        replacement: Value::Currency(Currency::from_scaled(
                            9223372036854775807_i64,
                        )),
                    },
                    Assignment {
                        row: 2,
                        expected: Value::Currency(Currency::from_scaled(9223372036854775807_i64)),
                        replacement: Value::Currency(Currency::from_scaled(
                            -9223372036854775808_i64,
                        )),
                    },
                ],
            )],
            Layout {
                definition: 110,
                data: 115,
                roots: [112, 113, 114],
                record_length: 23,
            },
            verify,
        ),
        load(
            (
                "Int64WidePrimary",
                "null-to-zero",
                "h6-wide-primary-before.accdb",
                "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
                "h6-Int64WidePrimary-null-to-zero.accdb",
                "41c0b84a468d846dd35e37253f9f82952dbbe7b699e21af1fef459ebf057807a",
                "54434e49303030348600000004000000005009000000000039a5165ab74b701f9e88b3aa5bcec4b1f0fb58cb1a2e86b7cfc481987eb39f2510000000010000000100000000000000496e743634576964655072696d6172790800000001000000000000004b657956616c75650100000000000000000000000000000000030000000000000000",
                1,
            ),
            vec![(
                "KeyValue",
                vec![Assignment {
                    row: 1,
                    expected: Value::Null,
                    replacement: Value::Int64(0_i64),
                }],
            )],
            Layout {
                definition: 120,
                data: 126,
                roots: [122, 123, 124],
                record_length: 23,
            },
            verify,
        ),
        load(
            (
                "Int64WidePrimary",
                "present-to-null",
                "h6-wide-primary-before.accdb",
                "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
                "h6-Int64WidePrimary-present-to-null.accdb",
                "c80420631ed5d4a0391d1cc9628bd7296d01fd3e43a3ce44a3d3be76dcc7f696",
                "54434e49303030348600000004000000005009000000000039a5165ab74b701f9e88b3aa5bcec4b1f0fb58cb1a2e86b7cfc481987eb39f2510000000010000000100000000000000496e743634576964655072696d6172790800000001000000000000004b657956616c75650000000000000000030000000000000080000000000000000000",
                1,
            ),
            vec![(
                "KeyValue",
                vec![Assignment {
                    row: 0,
                    expected: Value::Int64(-9223372036854775808_i64),
                    replacement: Value::Null,
                }],
            )],
            Layout {
                definition: 120,
                data: 126,
                roots: [122, 123, 124],
                record_length: 23,
            },
            verify,
        ),
        load(
            (
                "Int64WidePrimary",
                "swap-extremes",
                "h6-wide-primary-before.accdb",
                "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
                "h6-Int64WidePrimary-swap-extremes.accdb",
                "85887303d0536d5ecc1c8377b47faa0687da8cd0ca8c329068c9e8048a1fd2b0",
                "54434e4930303034a000000004000000005009000000000039a5165ab74b701f9e88b3aa5bcec4b1f0fb58cb1a2e86b7cfc481987eb39f2510000000010000000200000000000000496e743634576964655072696d6172790800000002000000000000004b657956616c7565000000000000000003000000000000008003ffffffffffffff7f020000000000000003ffffffffffffff7f030000000000000080",
                2,
            ),
            vec![(
                "KeyValue",
                vec![
                    Assignment {
                        row: 0,
                        expected: Value::Int64(-9223372036854775808_i64),
                        replacement: Value::Int64(9223372036854775807_i64),
                    },
                    Assignment {
                        row: 2,
                        expected: Value::Int64(9223372036854775807_i64),
                        replacement: Value::Int64(-9223372036854775808_i64),
                    },
                ],
            )],
            Layout {
                definition: 120,
                data: 126,
                roots: [122, 123, 124],
                record_length: 23,
            },
            verify,
        ),
        load(
            (
                "Int64WidePrimary",
                "primary-new-minus-one",
                "h6-wide-primary-before.accdb",
                "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
                "h6-Int64WidePrimary-primary-new-minus-one.accdb",
                "0ade471b5fac90982f7930aa0172d200d54e29990e5dbc6df48c4e2608d1d55d",
                "54434e49303030348000000004000000005009000000000039a5165ab74b701f9e88b3aa5bcec4b1f0fb58cb1a2e86b7cfc481987eb39f2510000000010000000100000000000000496e743634576964655072696d6172790200000001000000000000004964000000000000000003000000000000008003ffffffffffffffff",
                1,
            ),
            vec![(
                "Id",
                vec![Assignment {
                    row: 0,
                    expected: Value::Int64(-9223372036854775808_i64),
                    replacement: Value::Int64(-1_i64),
                }],
            )],
            Layout {
                definition: 120,
                data: 126,
                roots: [122, 123, 124],
                record_length: 23,
            },
            verify,
        ),
        load(
            (
                "Int64WidePrimary",
                "primary-swap-extremes",
                "h6-wide-primary-before.accdb",
                "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
                "h6-Int64WidePrimary-primary-swap-extremes.accdb",
                "72c582719c6ae0252e3a61998859f5b4c271a4d4392a2dd5dc10fd1124b290b8",
                "54434e49303030349a00000004000000005009000000000039a5165ab74b701f9e88b3aa5bcec4b1f0fb58cb1a2e86b7cfc481987eb39f2510000000010000000200000000000000496e743634576964655072696d6172790200000002000000000000004964000000000000000003000000000000008003ffffffffffffff7f020000000000000003ffffffffffffff7f030000000000000080",
                2,
            ),
            vec![(
                "Id",
                vec![
                    Assignment {
                        row: 0,
                        expected: Value::Int64(-9223372036854775808_i64),
                        replacement: Value::Int64(9223372036854775807_i64),
                    },
                    Assignment {
                        row: 2,
                        expected: Value::Int64(9223372036854775807_i64),
                        replacement: Value::Int64(-9223372036854775808_i64),
                    },
                ],
            )],
            Layout {
                definition: 120,
                data: 126,
                roots: [122, 123, 124],
                record_length: 23,
            },
            verify,
        ),
        load(
            (
                "CurrencyWidePrimary",
                "null-to-zero",
                "h6-wide-primary-before.accdb",
                "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
                "h6-CurrencyWidePrimary-null-to-zero.accdb",
                "70fc3b7d7d8b2944ec17cd8934c74e295b617db7bbbbf987d0d3c622868ea789",
                "54434e49303030348900000004000000005009000000000039a5165ab74b701f9e88b3aa5bcec4b1f0fb58cb1a2e86b7cfc481987eb39f251300000001000000010000000000000043757272656e6379576964655072696d6172790800000001000000000000004b657956616c75650100000000000000000000000000000000040000000000000000",
                1,
            ),
            vec![(
                "KeyValue",
                vec![Assignment {
                    row: 1,
                    expected: Value::Null,
                    replacement: Value::Currency(Currency::from_scaled(0_i64)),
                }],
            )],
            Layout {
                definition: 128,
                data: 133,
                roots: [130, 131, 132],
                record_length: 23,
            },
            verify,
        ),
        load(
            (
                "CurrencyWidePrimary",
                "present-to-null",
                "h6-wide-primary-before.accdb",
                "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
                "h6-CurrencyWidePrimary-present-to-null.accdb",
                "205ed33cea69acd161bac5de7147204f3e1032bcdd6e5de100c806d6c3360d0d",
                "54434e49303030348900000004000000005009000000000039a5165ab74b701f9e88b3aa5bcec4b1f0fb58cb1a2e86b7cfc481987eb39f251300000001000000010000000000000043757272656e6379576964655072696d6172790800000001000000000000004b657956616c75650000000000000000040000000000000080000000000000000000",
                1,
            ),
            vec![(
                "KeyValue",
                vec![Assignment {
                    row: 0,
                    expected: Value::Currency(Currency::from_scaled(-9223372036854775808_i64)),
                    replacement: Value::Null,
                }],
            )],
            Layout {
                definition: 128,
                data: 133,
                roots: [130, 131, 132],
                record_length: 23,
            },
            verify,
        ),
        load(
            (
                "CurrencyWidePrimary",
                "swap-extremes",
                "h6-wide-primary-before.accdb",
                "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
                "h6-CurrencyWidePrimary-swap-extremes.accdb",
                "64640cae02cdf397152b4a1a6efbd845307284b0a03cca90702300d995008cef",
                "54434e4930303034a300000004000000005009000000000039a5165ab74b701f9e88b3aa5bcec4b1f0fb58cb1a2e86b7cfc481987eb39f251300000001000000020000000000000043757272656e6379576964655072696d6172790800000002000000000000004b657956616c7565000000000000000004000000000000008004ffffffffffffff7f020000000000000004ffffffffffffff7f040000000000000080",
                2,
            ),
            vec![(
                "KeyValue",
                vec![
                    Assignment {
                        row: 0,
                        expected: Value::Currency(Currency::from_scaled(-9223372036854775808_i64)),
                        replacement: Value::Currency(Currency::from_scaled(
                            9223372036854775807_i64,
                        )),
                    },
                    Assignment {
                        row: 2,
                        expected: Value::Currency(Currency::from_scaled(9223372036854775807_i64)),
                        replacement: Value::Currency(Currency::from_scaled(
                            -9223372036854775808_i64,
                        )),
                    },
                ],
            )],
            Layout {
                definition: 128,
                data: 133,
                roots: [130, 131, 132],
                record_length: 23,
            },
            verify,
        ),
        load(
            (
                "CurrencyWidePrimary",
                "primary-new-minus-one",
                "h6-wide-primary-before.accdb",
                "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
                "h6-CurrencyWidePrimary-primary-new-minus-one.accdb",
                "b6912c11070c83352fc435804e2b028cf2fe89ad8ad2b530362cd47ac2bce391",
                "54434e49303030348300000004000000005009000000000039a5165ab74b701f9e88b3aa5bcec4b1f0fb58cb1a2e86b7cfc481987eb39f251300000001000000010000000000000043757272656e6379576964655072696d6172790200000001000000000000004964000000000000000004000000000000008004ffffffffffffffff",
                1,
            ),
            vec![(
                "Id",
                vec![Assignment {
                    row: 0,
                    expected: Value::Currency(Currency::from_scaled(-9223372036854775808_i64)),
                    replacement: Value::Currency(Currency::from_scaled(-1_i64)),
                }],
            )],
            Layout {
                definition: 128,
                data: 133,
                roots: [130, 131, 132],
                record_length: 23,
            },
            verify,
        ),
        load(
            (
                "CurrencyWidePrimary",
                "primary-swap-extremes",
                "h6-wide-primary-before.accdb",
                "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
                "h6-CurrencyWidePrimary-primary-swap-extremes.accdb",
                "6497c6624ae0cd9390065b78cbe07f9cb05b45941ed39efd41702b11af7b9ef2",
                "54434e49303030349d00000004000000005009000000000039a5165ab74b701f9e88b3aa5bcec4b1f0fb58cb1a2e86b7cfc481987eb39f251300000001000000020000000000000043757272656e6379576964655072696d6172790200000002000000000000004964000000000000000004000000000000008004ffffffffffffff7f020000000000000004ffffffffffffff7f040000000000000080",
                2,
            ),
            vec![(
                "Id",
                vec![
                    Assignment {
                        row: 0,
                        expected: Value::Currency(Currency::from_scaled(-9223372036854775808_i64)),
                        replacement: Value::Currency(Currency::from_scaled(
                            9223372036854775807_i64,
                        )),
                    },
                    Assignment {
                        row: 2,
                        expected: Value::Currency(Currency::from_scaled(9223372036854775807_i64)),
                        replacement: Value::Currency(Currency::from_scaled(
                            -9223372036854775808_i64,
                        )),
                    },
                ],
            )],
            Layout {
                definition: 128,
                data: 133,
                roots: [130, 131, 132],
                record_length: 23,
            },
            verify,
        ),
    ]
}
