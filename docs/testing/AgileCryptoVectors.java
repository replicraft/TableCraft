import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.Arrays;
import java.util.HexFormat;
import javax.crypto.Cipher;
import javax.crypto.spec.IvParameterSpec;
import javax.crypto.spec.SecretKeySpec;

/** Original deterministic public test vectors; this is not a file-encryption tool. */
class AgileCryptoVectors {
    static byte[] hash(String algorithm, byte[]... inputs) throws Exception {
        MessageDigest d = MessageDigest.getInstance(algorithm);
        for (byte[] input : inputs) d.update(input);
        return d.digest();
    }
    static byte[] key(String algorithm, byte[] iterated, byte[] block, int bits) throws Exception {
        byte[] h = hash(algorithm, iterated, block);
        byte[] key = new byte[bits / 8];
        Arrays.fill(key, (byte)0x36);
        System.arraycopy(h, 0, key, 0, Math.min(key.length, h.length));
        return key;
    }
    static byte[] encrypt(byte[] key, byte[] iv, byte[] plain) throws Exception {
        Cipher c = Cipher.getInstance("AES/CBC/NoPadding");
        c.init(Cipher.ENCRYPT_MODE, new SecretKeySpec(key, "AES"), new IvParameterSpec(iv));
        return c.doFinal(plain);
    }
    public static void main(String[] args) throws Exception {
        String[] passwordHashes = {"SHA-1", "SHA-256", "SHA-384", "SHA-512"};
        String[] dataHashes = {"SHA-512", "SHA-384", "SHA-256", "SHA-1"};
        int[] passwordBits = {192, 128, 256, 256};
        int[] dataBits = {256, 192, 128, 256};
        HexFormat hex = HexFormat.of();
        byte[] inputBlock = hex.parseHex("fea7d2763b4b9e79");
        byte[] hashBlock = hex.parseHex("d7aa0f6d3061344e");
        byte[] keyBlock = hex.parseHex("146e0be7abacd0d6");
        byte[] password = "A😀é".getBytes(StandardCharsets.UTF_16LE);
        byte[] salt = new byte[16], dataSalt = new byte[16], verifier = new byte[16];
        for (int i=0; i<16; i++) { salt[i]=(byte)i; dataSalt[i]=(byte)(16+i); verifier[i]=(byte)(32+i); }
        for (int test=0; test<4; test++) {
            String algorithm = passwordHashes[test];
            byte[] h = hash(algorithm, salt, password);
            for (int i=0; i<1000; i++) h=hash(algorithm, ByteBuffer.allocate(4).order(ByteOrder.LITTLE_ENDIAN).putInt(i).array(), h);
            byte[] verifierHash = hash(algorithm, verifier);
            verifierHash = Arrays.copyOf(verifierHash, (verifierHash.length+15)/16*16);
            byte[] fileKey = new byte[dataBits[test]/8];
            for (int i=0; i<fileKey.length; i++) fileKey[i]=(byte)(64+i);
            byte[] encryptedVerifier = encrypt(key(algorithm,h,inputBlock,passwordBits[test]),salt,verifier);
            byte[] encryptedHash = encrypt(key(algorithm,h,hashBlock,passwordBits[test]),salt,verifierHash);
            byte[] encryptedKey = encrypt(key(algorithm,h,keyBlock,passwordBits[test]),salt,Arrays.copyOf(fileKey,(fileKey.length+15)/16*16));
            byte[] pageBlock = {0x10,0x22,0x33,0x44}; // page 1 XOR native encoding key 11 22 33 44
            byte[] iv = Arrays.copyOf(hash(dataHashes[test],dataSalt,pageBlock),16);
            byte[] page = encrypt(fileKey,iv,hex.parseHex("6bc1bee22e409f96e93d7e117393172a"));
            System.out.println(algorithm+"\t"+hex.formatHex(encryptedVerifier)+"\t"+hex.formatHex(encryptedHash)+"\t"+hex.formatHex(encryptedKey)+"\t"+hex.formatHex(page));
        }
    }
}
