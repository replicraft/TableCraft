//! Original deterministic test-only encryption, never a production writer.
//! Header-retagged derivatives test codec routing, not native format creation.
use aes::cipher::{BlockModeEncrypt, KeyIvInit, block_padding::NoPadding};
use base64::{Engine, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};

fn encoded(bytes: &[u8]) -> String {
    let mut out = [0; 128];
    let n = STANDARD.encode_slice(bytes, &mut out).unwrap();
    std::str::from_utf8(&out[..n]).unwrap().into()
}
fn encrypt(key: &[u8], iv: &[u8], plain: &[u8]) -> Vec<u8> {
    let mut out = plain.to_vec();
    let n = out.len();
    cbc::Encryptor::<aes::Aes128>::new_from_slices(key, iv)
        .unwrap()
        .encrypt_padded::<NoPadding>(&mut out, n)
        .unwrap();
    out
}
pub fn encrypted_image(plain: &[u8], code: u8, password: &str) -> Vec<u8> {
    let data_salt = [0x11; 16];
    let password_salt = [0x22; 16];
    let file_key = [0x33; 16];
    let verifier = [0x44; 16];
    let encoding_key = [0x55, 0x66, 0x77, 0x88];
    let password: Vec<_> = password.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut digest = Sha256::new();
    digest.update(password_salt);
    digest.update(password);
    let initial = digest.finalize();
    let mut digest = Sha256::new();
    digest.update(0_u32.to_le_bytes());
    digest.update(initial);
    let iterated = digest.finalize();
    let key = |block: &[u8]| {
        let mut d = Sha256::new();
        d.update(iterated);
        d.update(block);
        d.finalize()[..16].to_vec()
    };
    let input = encrypt(
        &key(&[0xfe, 0xa7, 0xd2, 0x76, 0x3b, 0x4b, 0x9e, 0x79]),
        &password_salt,
        &verifier,
    );
    let hash = encrypt(
        &key(&[0xd7, 0xaa, 0x0f, 0x6d, 0x30, 0x61, 0x34, 0x4e]),
        &password_salt,
        &Sha256::digest(verifier),
    );
    let value = encrypt(
        &key(&[0x14, 0x6e, 0x0b, 0xe7, 0xab, 0xac, 0xd0, 0xd6]),
        &password_salt,
        &file_key,
    );
    let common = |salt: &[u8]| {
        format!(
            "saltSize=\"16\" blockSize=\"16\" keyBits=\"128\" hashSize=\"32\" cipherAlgorithm=\"AES\" cipherChaining=\"ChainingModeCBC\" hashAlgorithm=\"SHA256\" saltValue=\"{}\"",
            encoded(salt)
        )
    };
    let xml = format!(
        "<encryption xmlns=\"http://schemas.microsoft.com/office/2006/encryption\" xmlns:p=\"http://schemas.microsoft.com/office/2006/keyEncryptor/password\"><keyData {}/><keyEncryptors><keyEncryptor uri=\"http://schemas.microsoft.com/office/2006/keyEncryptor/password\"><p:encryptedKey {} spinCount=\"1\" encryptedVerifierHashInput=\"{}\" encryptedVerifierHashValue=\"{}\" encryptedKeyValue=\"{}\"/></keyEncryptor></keyEncryptors></encryption>",
        common(&data_salt),
        common(&password_salt),
        encoded(&input),
        encoded(&hash),
        encoded(&value)
    );
    let mut out = plain.to_vec();
    out[20] = code;
    out[21] = 1;
    for (i, b) in encoding_key.iter().enumerate() {
        out[62 + i] = [0xfb, 0x8a, 0xbc, 0x4e][i] ^ b;
    }
    let descriptor_length = 8 + xml.len();
    out[0x299..0x29b].copy_from_slice(&(descriptor_length as u16).to_le_bytes());
    out[0x29b..0x2a3].copy_from_slice(&[4, 0, 4, 0, 0x40, 0, 0, 0]);
    out[0x2a3..0x2a3 + xml.len()].copy_from_slice(xml.as_bytes());
    for (i, page) in out.as_chunks_mut::<4096>().0.iter_mut().enumerate().skip(1) {
        let mut ordinal = (i as u32).to_le_bytes();
        for (o, k) in ordinal.iter_mut().zip(encoding_key) {
            *o ^= k;
        }
        let mut h = Sha256::new();
        h.update(data_salt);
        h.update(ordinal);
        let iv = h.finalize();
        let encrypted = encrypt(&file_key, &iv[..16], page);
        page.copy_from_slice(&encrypted);
    }
    out
}
