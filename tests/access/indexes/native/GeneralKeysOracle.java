package com.healthmarketscience.jackcess.impl;

import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.HexFormat;
import java.util.Random;

/** Original deterministic key corpus; invokes the independent reference encoder. */
public class GeneralKeysOracle {
    private static byte[] key(GeneralLegacyIndexCodes encoder, String value, boolean ascending) throws Exception {
        var output = new ByteUtil.ByteStream();
        encoder.writeNonNullIndexTextValue(value, output, ascending);
        return output.toByteArray();
    }
    private static String units(String value) {
        var output = new StringBuilder();
        for (char unit : value.toCharArray()) output.append(String.format("%04x", (int) unit));
        return output.length() == 0 ? "-" : output.toString();
    }
    public static void main(String[] args) throws Exception {
        var encoders = new GeneralLegacyIndexCodes[]{GeneralLegacyIndexCodes.GEN_LEG_INSTANCE, GeneralIndexCodes.GEN_INSTANCE};
        if (args.length == 1 && args[0].equals("--exhaustive")) {
            for (int family = 0; family < encoders.length; family++) {
                for (int block = 0; block < 256; block++) {
                    var digest = MessageDigest.getInstance("SHA-256");
                    for (int code = block * 256; code < (block + 1) * 256; code++) {
                        for (boolean ascending : new boolean[]{true, false}) {
                            byte[] value = key(encoders[family], String.valueOf((char) code), ascending);
                            digest.update(new byte[]{(byte) (code >>> 8), (byte) code, (byte) (value.length >>> 8), (byte) value.length});
                            digest.update(value);
                        }
                    }
                    System.out.printf("%d\t%02x\t%s%n", family, block, HexFormat.of().formatHex(digest.digest()));
                }
            }
            return;
        }
        var cases = new ArrayList<String>();
        java.util.Collections.addAll(cases, "", "a", "A", "é", "e\u0301", "ß", "ss", "Œ", "æ", "a-b", "a'b", "a\u0001b", "\u0000", "\u0301", "\u0301kä", "\u0301\u0308", "😀", "\ud800", "\ud840", "\ud880", "\udb80", "\udc00", "\udc08", "\udfff", "カタカナ", "かな", "漢字々", "々", "漢々々", "カーカー", "ー", "カヽヾ", "\u0651", "ب\u0651", "\u06d6\u06d7\ufd24\ufd6f", "\u06d6".repeat(3), "\ufd24".repeat(5), "\u0eb0\u0eb1\u0ecb", "カ".repeat(8), "a  ", " ".repeat(260), "a".repeat(254) + "😀", "a".repeat(255) + "xyz", "a".repeat(254) + "  xyz");
        var random = new Random(20261020);
        for (int number = 0; number < 96; number++) {
            int length = 1 + random.nextInt(400);
            char[] value = new char[length];
            for (int index = 0; index < length; index++) value[index] = (char) random.nextInt(65536);
            cases.add(new String(value));
        }
        for (int family = 0; family < encoders.length; family++) {
            for (String value : cases) {
                System.out.printf("%d\t%s\t%s\t%s%n", family, units(value),
                        HexFormat.of().formatHex(key(encoders[family], value, true)),
                        HexFormat.of().formatHex(key(encoders[family], value, false)));
            }
        }
    }
}
