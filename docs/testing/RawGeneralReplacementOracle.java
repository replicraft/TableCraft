package com.healthmarketscience.jackcess.impl;

/** Original bounded raw replacement model over independent short General keys. */
public class RawGeneralReplacementOracle {
    static String replace(GeneralLegacyIndexCodes encoder, String source, String find,
                          String replacement, int start, int count) throws Exception {
        String remaining = source.substring(Math.min(start - 1, source.length()));
        if (find.isEmpty() || count == 0) {
            return remaining;
        }
        var output = new StringBuilder();
        while (!remaining.isEmpty() && count != 0) {
            int[] span = GeneralSearchOracle.span(encoder, remaining, find, 0, false);
            if (span == null) {
                break;
            }
            if (span[1] <= span[0]) {
                throw new AssertionError("nonadvancing independent match");
            }
            output.append(remaining, 0, span[0]).append(replacement);
            remaining = remaining.substring(span[1]);
            if (count > 0) {
                count--;
            }
        }
        return output.append(remaining).toString();
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 1 || !(args[0].equals("0") || args[0].equals("1"))) {
            throw new IllegalArgumentException("Expected General version 0/1");
        }
        var encoder = args[0].equals("0")
            ? GeneralLegacyIndexCodes.GEN_LEG_INSTANCE
            : GeneralIndexCodes.GEN_INSTANCE;
        String high = RawGeneralSearchOracle.text(0xd800);
        String low = RawGeneralSearchOracle.text(0xdfff);
        String pair = RawGeneralSearchOracle.text(0xd83d, 0xde00);
        String[][] cases = {
            {high + "SS ß", "ss", low},
            {high + "Straße", "ss", high},
            {pair, RawGeneralSearchOracle.text(0xd83d), ""},
            {pair, RawGeneralSearchOracle.text(0xde00), high},
            {high + "aaaa", "aa", low},
            {high + "a ", "A ", low},
            {high + "e\u0301É", "é", low},
            {high + "カーカ", "カ", high + low},
            {"", high, low},
            {high, "", low},
            {RawGeneralSearchOracle.text(0, 0xd800, 32), high, low},
            {"x" + low, "x", high}
        };
        for (String[] values : cases) {
            String source = values[0], find = values[1], replacement = values[2];
            for (int start = 1; start <= source.length() + 1; start++) {
                for (int count : new int[]{-1, 0, 1, 2}) {
                    String expected = replace(encoder, source, find, replacement, start, count);
                    System.out.printf("%s\t%s\t%s\t%d\t%d\t%s%n",
                        RawGeneralSearchOracle.hex(source), RawGeneralSearchOracle.hex(find),
                        RawGeneralSearchOracle.hex(replacement), start, count,
                        RawGeneralSearchOracle.hex(expected));
                }
            }
        }
    }
}
