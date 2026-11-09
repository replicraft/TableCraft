package com.healthmarketscience.jackcess.impl;

import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.HexFormat;
import java.util.List;
import java.util.Random;

/** Original oracle driver; calls the pinned independent implementation only. */
public final class FrenchKeysOracle {
    private static final short LCID = 1036;
    private static final short VARIANT = 0;
    private static final short VERSION = 0;
    private static final int BMP_BLOCKS = 256;
    private static final int UNITS_PER_BLOCK = 256;
    private static final int RANDOM_CASES = 128;
    private static final int RANDOM_MAX_UNITS = 400;
    private static final long SEED = 20261026L;
    private static final int KEY_LENGTH_BYTES = 2;
    private static final int MAX_KEY_BYTES = (1 << (Byte.SIZE * KEY_LENGTH_BYTES)) - 1;

    private static byte[] key(GeneralLegacyIndexCodes encoder, String value,
            boolean ascending) throws Exception {
        ByteUtil.ByteStream output = new ByteUtil.ByteStream();
        encoder.writeNonNullIndexTextValue(value, output, ascending);
        byte[] result = output.toByteArray();
        if (result.length > MAX_KEY_BYTES) {
            throw new AssertionError("Oracle framing length exceeded");
        }
        return result;
    }

    private static String units(String value) {
        StringBuilder output = new StringBuilder();
        for (char unit : value.toCharArray()) {
            output.append(String.format("%04x", (int) unit));
        }
        return output.length() == 0 ? "-" : output.toString();
    }

    private static List<String> cases() {
        List<String> cases = new ArrayList<>(List.of(
            "", "cote", "coté", "côte", "côté", "COTE", "cote  ",
            "é", "e\u0301", "è", "ê", "ë", "É", "a-b", "a'b", "a b",
            "a.b", "\u0000", "a\u0000b", "a\u0001b", "œ", "oe", "Œ", "æ", "ae", "ß", "ss",
            "\u0301", "\u0301\u0308", "\u0301kä", "a\u0301\u0308", "\u0301a\u0308",
            "😀", "\ud800", "\udc00", "a".repeat(254) + "😀",
            "カタカナ", "かな", "漢字々", "々", "漢々々", "カーカー", "ー", "カヽヾ",
            "\u0651", "ب\u0651", "\u06d6\u06d7\ufd24\ufd6f", "\u06d6".repeat(3),
            "\ufd24".repeat(5), "\u0eb0\u0eb1\u0ecb", "カ".repeat(8),
            "a".repeat(255), "à".repeat(255), "a".repeat(254) + "é",
            "a".repeat(254) + "è", "a".repeat(255) + "xyz", " ".repeat(260),
            "a".repeat(254) + "  xyz"
        ));
        for (int neutral = 0; neutral <= 16; neutral++) {
            String span = "a".repeat(neutral);
            cases.add("é" + span + "è");
            cases.add("e\u0301" + span + "e\u0300");
            cases.add(span + "é" + span);
            cases.add("œ" + span + "é");
            cases.add("é" + span + "œ");
            cases.add("é" + span + "カーカ");
            cases.add("é" + span + "a\u0001b");
        }
        for (int length : new int[]{1, 2, 3, 7, 8, 16, 127, 254, 255, 256, 400}) {
            cases.add("é".repeat(length));
            cases.add("e\u0301".repeat(length));
            cases.add("œé".repeat(length));
            cases.add("\ufd24é".repeat(length));
        }
        Random random = new Random(SEED);
        for (int number = 0; number < RANDOM_CASES; number++) {
            char[] value = new char[1 + random.nextInt(RANDOM_MAX_UNITS)];
            for (int position = 0; position < value.length; position++) {
                value[position] = (char) random.nextInt(Character.MAX_VALUE + 1);
            }
            cases.add(new String(value));
        }
        return cases;
    }

    public static void main(String[] args) throws Exception {
        ColumnImpl.SortOrder order = new ColumnImpl.SortOrder(LCID, VARIANT, VERSION);
        GeneralLegacyIndexCodes encoder = IndexCodes.getTextIndexCodes(order);
        if (encoder == null || !encoder.getClass().getSimpleName().equals("FrenchIndexCodes")) {
            throw new AssertionError("Exact descriptor did not resolve to French encoder");
        }
        if (args.length == 1 && args[0].equals("--exhaustive")) {
            for (int block = 0; block < BMP_BLOCKS; block++) {
                MessageDigest digest = MessageDigest.getInstance("SHA-256");
                for (int unit = block * UNITS_PER_BLOCK; unit < (block + 1) * UNITS_PER_BLOCK; unit++) {
                    for (boolean ascending : new boolean[]{true, false}) {
                        byte[] encoded = key(encoder, String.valueOf((char) unit), ascending);
                        digest.update(new byte[]{(byte) (unit >>> Byte.SIZE), (byte) unit,
                            (byte) (encoded.length >>> Byte.SIZE), (byte) encoded.length});
                        digest.update(encoded);
                    }
                }
                System.out.printf("%02x\t%s%n", block, HexFormat.of().formatHex(digest.digest()));
            }
        } else if (args.length == 1 && args[0].equals("--vectors")) {
            for (String value : cases()) {
                System.out.printf("%s\t%s\t%s%n", units(value),
                    HexFormat.of().formatHex(key(encoder, value, true)),
                    HexFormat.of().formatHex(key(encoder, value, false)));
            }
        } else {
            throw new IllegalArgumentException("Expected --exhaustive or --vectors");
        }
    }
}
