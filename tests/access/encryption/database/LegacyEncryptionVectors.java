import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.Arrays;
import java.util.HexFormat;
import javax.crypto.Cipher;
import javax.crypto.spec.SecretKeySpec;

/** Original JCE/BC known-answer generator; public fixed inputs, no database content. */
class LegacyEncryptionVectors {
    static byte[] hash(String name, byte[]... parts) throws Exception {
        var digest = MessageDigest.getInstance(name);
        for (byte[] part : parts) digest.update(part);
        return digest.digest();
    }
    static byte[] le(int number) {
        return ByteBuffer.allocate(4).order(ByteOrder.LITTLE_ENDIAN).putInt(number).array();
    }
    static byte[] key(String scheme, int bits, byte[] base, byte[] block) throws Exception {
        if (scheme.equals("Jet")) return block;
        if (scheme.equals("Binary")) return hash("MD5", base, block);
        byte[] digest = hash("SHA-1", base, block);
        if (scheme.equals("CryptoAPI")) {
            return Arrays.copyOf(Arrays.copyOf(digest, bits / 8), bits == 40 ? 16 : bits / 8);
        }
        byte[] inner = new byte[64], outer = new byte[64];
        Arrays.fill(inner, (byte)0x36); Arrays.fill(outer, (byte)0x5c);
        for (int i = 0; i < digest.length; ++i) { inner[i] ^= digest[i]; outer[i] ^= digest[i]; }
        byte[] first = hash("SHA-1", inner), second = hash("SHA-1", outer);
        byte[] combined = Arrays.copyOf(first, first.length + second.length);
        System.arraycopy(second, 0, combined, first.length, second.length);
        return Arrays.copyOf(combined, bits / 8);
    }
    public static void main(String[] args) throws Exception {
        java.security.Security.addProvider(new org.bouncycastle.jce.provider.BouncyCastleProvider());
        java.nio.file.Path directory = args.length == 0 ? null : java.nio.file.Path.of(args[0]);
        byte[] plain = null;
        if (directory != null) {
            java.nio.file.Files.createDirectory(directory);
            var path = directory.resolve("Plain.accdb");
            try (var db = new com.healthmarketscience.jackcess.DatabaseBuilder(path)
                    .setFileFormat(com.healthmarketscience.jackcess.Database.FileFormat.V2016).create()) {
                db.setEvaluateExpressions(false);
                var table = new com.healthmarketscience.jackcess.TableBuilder("Records")
                    .addColumn(new com.healthmarketscience.jackcess.ColumnBuilder("ID", com.healthmarketscience.jackcess.DataType.LONG))
                    .addColumn(new com.healthmarketscience.jackcess.ColumnBuilder("Label", com.healthmarketscience.jackcess.DataType.TEXT).setLengthInUnits(80)).toTable(db);
                table.addRow(1, "first"); table.addRow(2, "A😀é"); table.addRow(3, null); table.addRow(4, "");
            }
            plain = java.nio.file.Files.readAllBytes(path);
        }
        byte[] salt = HexFormat.of().parseHex("000102030405060708090a0b0c0d0e0f");
        byte[] verifier = HexFormat.of().parseHex("101112131415161718191a1b1c1d1e1f");
        byte[] password = "A😀é".getBytes(StandardCharsets.UTF_16LE);
        for (String scheme : new String[]{"Jet", "Binary", "CryptoAPI", "Standard", "Nonstandard"}) {
            int[] widths = scheme.equals("CryptoAPI") ? new int[]{40, 128} :
                scheme.equals("Standard") || scheme.equals("Nonstandard") ? new int[]{128, 192, 256} : new int[]{128};
            for (int bits : widths) {
                byte[] base;
                if (scheme.equals("Binary")) {
                    byte[] repeated = new byte[336], prefix = hash("MD5", password);
                    for (int i = 0; i < repeated.length; i += 21) {
                        System.arraycopy(prefix, 0, repeated, i, 5);
                        System.arraycopy(salt, 0, repeated, i + 5, 16);
                    }
                    base = Arrays.copyOf(hash("MD5", repeated), 5);
                } else {
                    base = hash("SHA-1", salt, password);
                    if (scheme.equals("Standard")) for (int i = 0; i < 50000; ++i) base = hash("SHA-1", le(i), base);
                }
                boolean aes = scheme.equals("Standard") || scheme.equals("Nonstandard");
                String cipherName = aes ? "AES/ECB/NoPadding" : "ARCFOUR";
                String algorithm = aes ? "AES" : "ARCFOUR";
                Cipher cipher = aes ? Cipher.getInstance(cipherName) : Cipher.getInstance(cipherName, "BC");
                cipher.init(Cipher.ENCRYPT_MODE, new SecretKeySpec(key(scheme, bits, base, le(0)), algorithm));
                byte[] encodedVerifier = cipher.update(verifier);
                byte[] verifierHash = hash(scheme.equals("Binary") ? "MD5" : "SHA-1", verifier);
                byte[] encodedHash = cipher.doFinal(aes ? Arrays.copyOf(verifierHash, 32) : verifierHash);
                cipher.init(Cipher.ENCRYPT_MODE, new SecretKeySpec(key(scheme, bits, base, le(0x88776654)), algorithm));
                byte[] prefix = cipher.doFinal(new byte[32]);
                System.out.printf("%s %d %s %s %s%n", scheme, bits, HexFormat.of().formatHex(encodedVerifier), HexFormat.of().formatHex(encodedHash), HexFormat.of().formatHex(prefix));
                if (plain != null && !scheme.equals("Jet")) {
                    byte[] result = plain.clone();
                    byte[] encodingKey = le(0x88776655);
                    for (int i = 0; i < encodingKey.length; ++i) result[62 + i] ^= encodingKey[i];
                    ByteBuffer info = ByteBuffer.allocate(1024).order(ByteOrder.LITTLE_ENDIAN);
                    if (scheme.equals("Binary")) info.putShort((short)1).putShort((short)1);
                    else {
                        int flags = scheme.equals("Standard") ? 0x24 : 4;
                        int algorithmId = scheme.equals("CryptoAPI") ? 0x6801 : switch(bits) { case 128 -> 0x660e; case 192 -> 0x660f; default -> 0x6610; };
                        info.putShort((short)4).putShort((short)2).putInt(flags).putInt(34);
                        info.putInt(flags).putInt(0).putInt(algorithmId).putInt(0x8004).putInt(bits).putInt(scheme.equals("CryptoAPI") ? 1 : 24).putInt(0).putInt(0).putShort((short)0).putInt(16);
                    }
                    info.put(salt).put(encodedVerifier);
                    if (!scheme.equals("Binary")) info.putInt(20);
                    info.put(encodedHash);
                    Arrays.fill(result, 0x299, 0xe00, (byte)0);
                    ByteBuffer.wrap(result).order(ByteOrder.LITTLE_ENDIAN).putShort(0x299, (short)info.position());
                    System.arraycopy(info.array(), 0, result, 0x29b, info.position());
                    for (int page = 1; page < result.length / 4096; ++page) {
                        cipher.init(Cipher.ENCRYPT_MODE, new SecretKeySpec(key(scheme, bits, base, le(page ^ 0x88776655)), algorithm));
                        byte[] encrypted = cipher.doFinal(plain, page * 4096, 4096);
                        System.arraycopy(encrypted, 0, result, page * 4096, encrypted.length);
                    }
                    for (int code : new int[]{2, 3, 5, 6}) {
                        result[20] = (byte)code;
                        java.nio.file.Files.write(directory.resolve(scheme + "-" + bits + "-" + code + ".accdb"), result, java.nio.file.StandardOpenOption.CREATE_NEW);
                    }
                }
            }
        }
    }
}
