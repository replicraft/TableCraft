package com.healthmarketscience.jackcess.impl;

/** Original raw-unit controls using the existing independent short-span model.
 * Only synthetic code units enter this helper; no native SQL executes. */
public class RawGeneralSearchOracle {
    static String text(int... units) {
        var value = new StringBuilder();
        for (int unit : units) {
            value.append((char) unit);
        }
        return value.toString();
    }

    static String hex(String value) {
        var encoded = new StringBuilder();
        for (int offset = 0; offset < value.length(); offset++) {
            encoded.append(String.format("%04x", (int) value.charAt(offset)));
        }
        return encoded.isEmpty() ? "-" : encoded.toString();
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 1 || !(args[0].equals("0") || args[0].equals("1"))) {
            throw new IllegalArgumentException("Expected General version 0/1");
        }
        var encoder = args[0].equals("0")
            ? GeneralLegacyIndexCodes.GEN_LEG_INSTANCE
            : GeneralIndexCodes.GEN_INSTANCE;
        String[] values = {
            "", "a", "A ", "ss", "ß", "é", "e\u0301", "カーカ",
            text(0), text(0xd800), text(0xdc00), text(0xd83d, 0xde00),
            text(0xd800, 65), text(0xd800, 223), text(0xd800, 65, 0xd800),
            text(0xdc00, 0xd800), text(0xd800, 0xd83d, 0xde00),
            text(0xffff), text(0, 0xd800, 32)
        };
        for (String haystack : values) {
            for (String needle : values) {
                for (boolean reverse : new boolean[]{false, true}) {
                    for (int start = 1; start <= haystack.length() + 1; start++) {
                        int expected = GeneralSearchOracle.position(encoder, haystack, needle, start, reverse);
                        System.out.printf("%s\t%s\t%s\t%d\t%d%n",
                            reverse ? "R" : "F", hex(haystack), hex(needle), start, expected);
                    }
                }
            }
        }
    }
}
