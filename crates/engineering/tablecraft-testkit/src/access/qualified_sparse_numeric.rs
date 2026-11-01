//! Pinned generic-read source images; action admission is separately requalified.
//! Native-gap and crossing-byte files are independently authored derivatives.
use std::{fs, path::PathBuf};
use tablecraft_core::values::{Currency, Value};

pub struct Case {
    pub header: u8,
    pub table: &'static str,
    pub family: &'static str,
    pub recipe: &'static str,
    pub operation: &'static str,
    pub definition: u32,
    pub data: u32,
    pub roots: [u32; 3],
    pub before: Vec<u8>,
    pub after: Vec<u8>,
}

impl Case {
    pub fn numbers(&self) -> [usize; 3] {
        match self.family {
            "native-gap" => [0, 3, 4],
            "stored-id-gap" => [0, 1, 2],
            "crossing-byte" => [0, 8, 9],
            _ => unreachable!(),
        }
    }

    pub fn stored_ids(&self) -> [u16; 3] {
        match self.family {
            "native-gap" | "stored-id-gap" => [0, 3, 4],
            "crossing-byte" => [0, 8, 9],
            _ => unreachable!(),
        }
    }

    pub fn maximum(&self) -> u16 {
        match self.family {
            "native-gap" => 5,
            "stored-id-gap" => 3,
            "crossing-byte" => 10,
            _ => unreachable!(),
        }
    }

    pub fn record_bytes(&self) -> usize {
        if self.family == "crossing-byte" {
            20
        } else {
            19
        }
    }

    pub fn record(&self, slot: usize) -> Vec<u8> {
        assert!(slot < 3);
        let mut record = self.maximum().to_le_bytes().to_vec();
        record.extend_from_slice(&(slot as i32 + 1).to_le_bytes());
        record.extend_from_slice(&(if slot == 2 { i64::MAX } else { i64::MIN }).to_le_bytes());
        record.extend_from_slice(&([17_i32, 34, 51][slot]).to_le_bytes());
        let present = slot != 1;
        match self.family {
            "native-gap" => record.push(if present { 0x19 } else { 0x11 }),
            "stored-id-gap" => record.push(if present { 7 } else { 5 }),
            "crossing-byte" => record
                .extend_from_slice(&(if present { 0x0301_u16 } else { 0x0201_u16 }).to_le_bytes()),
            _ => unreachable!(),
        }
        assert_eq!(record.len(), self.record_bytes());
        record
    }

    pub fn scalar(&self, units: i64) -> Value {
        if self.table.starts_with("Currency") {
            Value::Currency(Currency::from_scaled(units))
        } else {
            Value::Int64(units)
        }
    }

    pub fn request(&self) -> Vec<Value> {
        assert_eq!(self.operation, "insert");
        vec![
            Value::Int32(4),
            if self.recipe == "insert-null" {
                Value::Null
            } else {
                self.scalar(0)
            },
            Value::Int32(68),
        ]
    }

    pub fn original_rows(&self) -> Vec<Vec<Value>> {
        (0..3)
            .map(|slot| {
                vec![
                    Value::Int32(slot + 1),
                    if slot == 1 {
                        Value::Null
                    } else {
                        self.scalar(if slot == 0 { i64::MIN } else { i64::MAX })
                    },
                    Value::Int32([17, 34, 51][slot as usize]),
                ]
            })
            .collect()
    }

    pub fn rows(&self) -> Vec<Vec<Value>> {
        if self.operation == "delete" {
            return Vec::new();
        }
        let mut rows = self.original_rows();
        rows.push(self.request());
        rows
    }
}

/// All36 source pairs are verified before returning any matching request case.
/// The callback supplies the caller's hash implementation; no new dependency.
pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let mut result = Vec::new();
    for (
        env,
        header,
        table,
        family,
        recipe,
        definition,
        data,
        roots,
        before,
        oldhash,
        after,
        newhash,
    ) in [
        (
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            5_u8,
            "Int64Keys",
            "native-gap",
            "insert-null",
            89_u32,
            94_u32,
            [91, 92, 93],
            "h5-Int64Keys-sparse-before.accdb",
            "b5bbc821a49442bcbf876ea5fcb8c560b1825a2440f0e2c5d54c563e524ab89a",
            "h5-Int64Keys-sparse-insert-null.accdb",
            "a492018abad9b265e840c8871ab6903da0eb8ba1f97c6f9a7e73df414795b0eb",
        ),
        (
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            5_u8,
            "Int64Keys",
            "native-gap",
            "insert-zero",
            89_u32,
            94_u32,
            [91, 92, 93],
            "h5-Int64Keys-sparse-before.accdb",
            "b5bbc821a49442bcbf876ea5fcb8c560b1825a2440f0e2c5d54c563e524ab89a",
            "h5-Int64Keys-sparse-insert-zero.accdb",
            "bd98e35d1b2e7b9291613d970bc6fe4d3690231bfc58925ab0216ee6fdb24d94",
        ),
        (
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            5_u8,
            "Int64Keys",
            "native-gap",
            "delete-all",
            89_u32,
            94_u32,
            [91, 92, 93],
            "h5-Int64Keys-sparse-before.accdb",
            "b5bbc821a49442bcbf876ea5fcb8c560b1825a2440f0e2c5d54c563e524ab89a",
            "h5-Int64Keys-sparse-delete-all.accdb",
            "aec4044e7c1e4fb7d0954f3ce6a8d54853b8423f48743c7601ffd8d8013f009e",
        ),
        (
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            5_u8,
            "CurrencyKeys",
            "native-gap",
            "insert-null",
            96_u32,
            101_u32,
            [98, 99, 100],
            "h5-CurrencyKeys-sparse-before.accdb",
            "aa8b5d16838dab85df89b42fa61deb1dec0128e6b1408b2236e3d0cb2f93aa40",
            "h5-CurrencyKeys-sparse-insert-null.accdb",
            "5828142823e3af02372f2e99cfd9bc23922588e6428edefb6e1d717cd714c3ff",
        ),
        (
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            5_u8,
            "CurrencyKeys",
            "native-gap",
            "insert-zero",
            96_u32,
            101_u32,
            [98, 99, 100],
            "h5-CurrencyKeys-sparse-before.accdb",
            "aa8b5d16838dab85df89b42fa61deb1dec0128e6b1408b2236e3d0cb2f93aa40",
            "h5-CurrencyKeys-sparse-insert-zero.accdb",
            "4ead59aa08f76ce7a70ef194e1619f3711682875e4c6161301a22547e85170c1",
        ),
        (
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            5_u8,
            "CurrencyKeys",
            "native-gap",
            "delete-all",
            96_u32,
            101_u32,
            [98, 99, 100],
            "h5-CurrencyKeys-sparse-before.accdb",
            "aa8b5d16838dab85df89b42fa61deb1dec0128e6b1408b2236e3d0cb2f93aa40",
            "h5-CurrencyKeys-sparse-delete-all.accdb",
            "61d06206dcae924bee4dcd7ebd54ae10f122b223f3dde28802a34de96866857a",
        ),
        (
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            6_u8,
            "Int64Keys",
            "native-gap",
            "insert-null",
            106_u32,
            111_u32,
            [108, 109, 110],
            "h6-Int64Keys-sparse-before.accdb",
            "24d227706b7dd46a52c96b6790f24c716eb75075ffa99c7d82b239f6691f389f",
            "h6-Int64Keys-sparse-insert-null.accdb",
            "b7d775411cc19ca9ddb216dae4d85cd7d37c6c70643bd3e4004048ebd4a60965",
        ),
        (
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            6_u8,
            "Int64Keys",
            "native-gap",
            "insert-zero",
            106_u32,
            111_u32,
            [108, 109, 110],
            "h6-Int64Keys-sparse-before.accdb",
            "24d227706b7dd46a52c96b6790f24c716eb75075ffa99c7d82b239f6691f389f",
            "h6-Int64Keys-sparse-insert-zero.accdb",
            "cf87a6c34059dc212a9785e72ed0f7938fa3a9f3a15864eea22c6fb064275faf",
        ),
        (
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            6_u8,
            "Int64Keys",
            "native-gap",
            "delete-all",
            106_u32,
            111_u32,
            [108, 109, 110],
            "h6-Int64Keys-sparse-before.accdb",
            "24d227706b7dd46a52c96b6790f24c716eb75075ffa99c7d82b239f6691f389f",
            "h6-Int64Keys-sparse-delete-all.accdb",
            "4af73abe48d49d9d3df2c5e9ada345783a5dbacbc000d43853dad39865d3f04e",
        ),
        (
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            6_u8,
            "CurrencyKeys",
            "native-gap",
            "insert-null",
            113_u32,
            118_u32,
            [115, 116, 117],
            "h6-CurrencyKeys-sparse-before.accdb",
            "f2bca089d76ccf2b4c30510f886be9a84ef76df619fa106827b9ebbcf38e5c89",
            "h6-CurrencyKeys-sparse-insert-null.accdb",
            "f14349af5286d078307dd9270cb2335b9291abf544fb4c6fb44e715db072e8c6",
        ),
        (
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            6_u8,
            "CurrencyKeys",
            "native-gap",
            "insert-zero",
            113_u32,
            118_u32,
            [115, 116, 117],
            "h6-CurrencyKeys-sparse-before.accdb",
            "f2bca089d76ccf2b4c30510f886be9a84ef76df619fa106827b9ebbcf38e5c89",
            "h6-CurrencyKeys-sparse-insert-zero.accdb",
            "c5afd1793c08366e9dbe445ba493731d752b19d93cd8cdd3baea7b61986cc986",
        ),
        (
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            6_u8,
            "CurrencyKeys",
            "native-gap",
            "delete-all",
            113_u32,
            118_u32,
            [115, 116, 117],
            "h6-CurrencyKeys-sparse-before.accdb",
            "f2bca089d76ccf2b4c30510f886be9a84ef76df619fa106827b9ebbcf38e5c89",
            "h6-CurrencyKeys-sparse-delete-all.accdb",
            "0bdb05798fcb77c6468a864aa371a17e5384d49026556ae7d6ce620d7f90fd56",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            5_u8,
            "Int64StableIds",
            "stored-id-gap",
            "insert-null",
            117_u32,
            122_u32,
            [119, 120, 121],
            "h5-Int64StableIds-before.accdb",
            "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
            "h5-Int64StableIds-insert-null.accdb",
            "315bd7f996c57cd76656fb3c98b7d6f64b6e6bba27807d21d4bbddd332997bac",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            5_u8,
            "Int64StableIds",
            "stored-id-gap",
            "insert-zero",
            117_u32,
            122_u32,
            [119, 120, 121],
            "h5-Int64StableIds-before.accdb",
            "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
            "h5-Int64StableIds-insert-zero.accdb",
            "ece74c91673f2b0534a54d22b8b8583660ce441f86e820a6fb695b9627875c80",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            5_u8,
            "Int64StableIds",
            "stored-id-gap",
            "delete-all",
            117_u32,
            122_u32,
            [119, 120, 121],
            "h5-Int64StableIds-before.accdb",
            "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
            "h5-Int64StableIds-delete-all.accdb",
            "156de5a3fef54822c2ccbfc8ffeb58e3100ae9ae2e4bc5b03a9349321327cecb",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            5_u8,
            "CurrencyStableIds",
            "stored-id-gap",
            "insert-null",
            124_u32,
            130_u32,
            [126, 127, 128],
            "h5-CurrencyStableIds-before.accdb",
            "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
            "h5-CurrencyStableIds-insert-null.accdb",
            "4881c8aadf202e3f8696934e046abb27f4b5e0206172f7c1773d9fa1ca8d57e1",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            5_u8,
            "CurrencyStableIds",
            "stored-id-gap",
            "insert-zero",
            124_u32,
            130_u32,
            [126, 127, 128],
            "h5-CurrencyStableIds-before.accdb",
            "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
            "h5-CurrencyStableIds-insert-zero.accdb",
            "01b0d06bc96dd42f4e4d3c90ec4a1bd4ede79255ca675ecd3fffa0737fcca1c6",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            5_u8,
            "CurrencyStableIds",
            "stored-id-gap",
            "delete-all",
            124_u32,
            130_u32,
            [126, 127, 128],
            "h5-CurrencyStableIds-before.accdb",
            "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
            "h5-CurrencyStableIds-delete-all.accdb",
            "56dd7f21e26cee5b577af95c8f8bd65877aaa65dfe8f601c205c2462522891b1",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            6_u8,
            "Int64StableIds",
            "stored-id-gap",
            "insert-null",
            135_u32,
            140_u32,
            [137, 138, 139],
            "h6-Int64StableIds-before.accdb",
            "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
            "h6-Int64StableIds-insert-null.accdb",
            "9a0675d4e31cdbc14aeb6ae04e9cd6753b53493637f319883c8e3b5ed42fb1fa",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            6_u8,
            "Int64StableIds",
            "stored-id-gap",
            "insert-zero",
            135_u32,
            140_u32,
            [137, 138, 139],
            "h6-Int64StableIds-before.accdb",
            "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
            "h6-Int64StableIds-insert-zero.accdb",
            "1e233cc94264a93f13a6e60f7fe1613e7a212be82a24f5e1b5daf0eee8d91527",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            6_u8,
            "Int64StableIds",
            "stored-id-gap",
            "delete-all",
            135_u32,
            140_u32,
            [137, 138, 139],
            "h6-Int64StableIds-before.accdb",
            "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
            "h6-Int64StableIds-delete-all.accdb",
            "dfa822d3138f84991a4587f22a32aff123c0507d4701c0d87afc67df68059ac6",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            6_u8,
            "CurrencyStableIds",
            "stored-id-gap",
            "insert-null",
            142_u32,
            147_u32,
            [144, 145, 146],
            "h6-CurrencyStableIds-before.accdb",
            "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
            "h6-CurrencyStableIds-insert-null.accdb",
            "d4d3fb123cb682a15fdeb2b824fd7680acb5dae6602552f592b85b334d1b81b1",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            6_u8,
            "CurrencyStableIds",
            "stored-id-gap",
            "insert-zero",
            142_u32,
            147_u32,
            [144, 145, 146],
            "h6-CurrencyStableIds-before.accdb",
            "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
            "h6-CurrencyStableIds-insert-zero.accdb",
            "4144c1119e3ffff77163482fd094174988acf0486218d9a108cce2ea7fa6951e",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            6_u8,
            "CurrencyStableIds",
            "stored-id-gap",
            "delete-all",
            142_u32,
            147_u32,
            [144, 145, 146],
            "h6-CurrencyStableIds-before.accdb",
            "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
            "h6-CurrencyStableIds-delete-all.accdb",
            "5db2133516ed89e43123081aa12d63f639194febe27b413b814fb7525cb72310",
        ),
        (
            "TABLECRAFT_INTEGRAL_CROSS_BYTE_SOURCE_DIR",
            5_u8,
            "Int64Keys",
            "crossing-byte",
            "insert-null",
            89_u32,
            94_u32,
            [91, 92, 93],
            "h5-Int64Keys-crossing-byte-before.accdb",
            "a87b51409365f249ef9143cfd7b74d4c423ddbc20e9927244e0d925ba2b72dee",
            "h5-Int64Keys-crossing-byte-insert-null.accdb",
            "f52900c273cdcaca344903ab839cd7487b7aaaceeebe636123bf62a679aaf7e4",
        ),
        (
            "TABLECRAFT_INTEGRAL_CROSS_BYTE_SOURCE_DIR",
            5_u8,
            "Int64Keys",
            "crossing-byte",
            "insert-zero",
            89_u32,
            94_u32,
            [91, 92, 93],
            "h5-Int64Keys-crossing-byte-before.accdb",
            "a87b51409365f249ef9143cfd7b74d4c423ddbc20e9927244e0d925ba2b72dee",
            "h5-Int64Keys-crossing-byte-insert-zero.accdb",
            "7168bf5e0bd5e963eaa3547496d30d66c1ef37740950eaff4470171e7a5edd63",
        ),
        (
            "TABLECRAFT_INTEGRAL_CROSS_BYTE_SOURCE_DIR",
            5_u8,
            "Int64Keys",
            "crossing-byte",
            "delete-all",
            89_u32,
            94_u32,
            [91, 92, 93],
            "h5-Int64Keys-crossing-byte-before.accdb",
            "a87b51409365f249ef9143cfd7b74d4c423ddbc20e9927244e0d925ba2b72dee",
            "h5-Int64Keys-crossing-byte-delete-all.accdb",
            "4f4d2e7618793ae20f16bce05a03018ca3729d24c5664b92418544a83d170d2e",
        ),
        (
            "TABLECRAFT_INTEGRAL_CROSS_BYTE_SOURCE_DIR",
            5_u8,
            "CurrencyKeys",
            "crossing-byte",
            "insert-null",
            96_u32,
            101_u32,
            [98, 99, 100],
            "h5-CurrencyKeys-crossing-byte-before.accdb",
            "fabd24e78c5b547451d23bbf852b2a4d4f0fa1909c9dc2bfb3dd29caf526ba74",
            "h5-CurrencyKeys-crossing-byte-insert-null.accdb",
            "0c812fa64b88a2bc2c2d54eddc8247ad96778e4d36dab0bfea9dc2e6c26ad3c8",
        ),
        (
            "TABLECRAFT_INTEGRAL_CROSS_BYTE_SOURCE_DIR",
            5_u8,
            "CurrencyKeys",
            "crossing-byte",
            "insert-zero",
            96_u32,
            101_u32,
            [98, 99, 100],
            "h5-CurrencyKeys-crossing-byte-before.accdb",
            "fabd24e78c5b547451d23bbf852b2a4d4f0fa1909c9dc2bfb3dd29caf526ba74",
            "h5-CurrencyKeys-crossing-byte-insert-zero.accdb",
            "72fbeff20b068f5101da69c472c1c0e47a5803991168b4b20509f6534649ebc9",
        ),
        (
            "TABLECRAFT_INTEGRAL_CROSS_BYTE_SOURCE_DIR",
            5_u8,
            "CurrencyKeys",
            "crossing-byte",
            "delete-all",
            96_u32,
            101_u32,
            [98, 99, 100],
            "h5-CurrencyKeys-crossing-byte-before.accdb",
            "fabd24e78c5b547451d23bbf852b2a4d4f0fa1909c9dc2bfb3dd29caf526ba74",
            "h5-CurrencyKeys-crossing-byte-delete-all.accdb",
            "c3e09219fc24bd163e45cc356cdc79bec46af3e1da2a952ef21790ccb8431c1e",
        ),
        (
            "TABLECRAFT_INTEGRAL_CROSS_BYTE_SOURCE_DIR",
            6_u8,
            "Int64Keys",
            "crossing-byte",
            "insert-null",
            106_u32,
            111_u32,
            [108, 109, 110],
            "h6-Int64Keys-crossing-byte-before.accdb",
            "f012f483886d5258915f45420f1b77a056f0ec29f019047c73289829c4baec42",
            "h6-Int64Keys-crossing-byte-insert-null.accdb",
            "d23ec368894741b0cd563fb5d2232da902493db4e1ace367be0469abbed067bf",
        ),
        (
            "TABLECRAFT_INTEGRAL_CROSS_BYTE_SOURCE_DIR",
            6_u8,
            "Int64Keys",
            "crossing-byte",
            "insert-zero",
            106_u32,
            111_u32,
            [108, 109, 110],
            "h6-Int64Keys-crossing-byte-before.accdb",
            "f012f483886d5258915f45420f1b77a056f0ec29f019047c73289829c4baec42",
            "h6-Int64Keys-crossing-byte-insert-zero.accdb",
            "2f53c1c611bf58ba91ae6db92b02ecaf064cebbe3a3ca2e4bc715227bb139c7c",
        ),
        (
            "TABLECRAFT_INTEGRAL_CROSS_BYTE_SOURCE_DIR",
            6_u8,
            "Int64Keys",
            "crossing-byte",
            "delete-all",
            106_u32,
            111_u32,
            [108, 109, 110],
            "h6-Int64Keys-crossing-byte-before.accdb",
            "f012f483886d5258915f45420f1b77a056f0ec29f019047c73289829c4baec42",
            "h6-Int64Keys-crossing-byte-delete-all.accdb",
            "306275731861b6162cdf206ab2e10a9254e4e607c244e7bd9b4d8d96c0ee4ebe",
        ),
        (
            "TABLECRAFT_INTEGRAL_CROSS_BYTE_SOURCE_DIR",
            6_u8,
            "CurrencyKeys",
            "crossing-byte",
            "insert-null",
            113_u32,
            118_u32,
            [115, 116, 117],
            "h6-CurrencyKeys-crossing-byte-before.accdb",
            "1ada867caaa10aabb76f93f60ac906e0872bd5221f43920331f15cd6ed3bf13b",
            "h6-CurrencyKeys-crossing-byte-insert-null.accdb",
            "6bbd07f72d0770e3f7b1b64850a4020cc49b6ca0a5a30df72aa88dde6b3e41b3",
        ),
        (
            "TABLECRAFT_INTEGRAL_CROSS_BYTE_SOURCE_DIR",
            6_u8,
            "CurrencyKeys",
            "crossing-byte",
            "insert-zero",
            113_u32,
            118_u32,
            [115, 116, 117],
            "h6-CurrencyKeys-crossing-byte-before.accdb",
            "1ada867caaa10aabb76f93f60ac906e0872bd5221f43920331f15cd6ed3bf13b",
            "h6-CurrencyKeys-crossing-byte-insert-zero.accdb",
            "b3ae263ab54664e042644436053d0e04b4757394ad51a800b62b95e12382f92a",
        ),
        (
            "TABLECRAFT_INTEGRAL_CROSS_BYTE_SOURCE_DIR",
            6_u8,
            "CurrencyKeys",
            "crossing-byte",
            "delete-all",
            113_u32,
            118_u32,
            [115, 116, 117],
            "h6-CurrencyKeys-crossing-byte-before.accdb",
            "1ada867caaa10aabb76f93f60ac906e0872bd5221f43920331f15cd6ed3bf13b",
            "h6-CurrencyKeys-crossing-byte-delete-all.accdb",
            "9c09a7ec791531c8f610ff27ea3b9d405bbed992e28ad1fa0bb02724bb73a0e4",
        ),
    ] {
        let dir = PathBuf::from(std::env::var_os(env).unwrap());
        assert!(dir.is_absolute());
        let before = fs::read(dir.join(before)).unwrap();
        let after = fs::read(dir.join(after)).unwrap();
        verify(&before, oldhash);
        verify(&after, newhash);
        result.push(Case {
            header,
            table,
            family,
            recipe,
            operation: if recipe == "delete-all" {
                "delete"
            } else {
                "insert"
            },
            definition,
            data,
            roots,
            before,
            after,
        });
    }
    assert_eq!(result.len(), 36);
    result
}
