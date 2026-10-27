package com.healthmarketscience.jackcess.impl;

import java.util.ArrayList;
import java.util.HexFormat;
import java.util.List;
import java.util.Random;

/** Original reference caller. It exports measured metadata, never upstream code. */
public final class FrenchComparisonOracle {
    private static final short LCID = 1036;
    private static final short VARIANT = 0;
    private static final short VERSION = 0;
    private static final String ALPHABET = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZéèêëàâäîïôöùûüçÉÈÊËÀÂÄÎÏÔÖÙÛÜÇ";
    private static final int SHORT_LIMIT = 255;
    private static final int RANDOM_CASES = 128;
    private static final long RANDOM_SEED = 20261026L;

    private static String hex(byte[] value) {
        return value == null || value.length == 0 ? "-" : HexFormat.of().formatHex(value);
    }

    private static String units(String value) {
        StringBuilder output = new StringBuilder();
        for (char unit : value.toCharArray()) {
            output.append(String.format("%04x", (int) unit));
        }
        return output.length() == 0 ? "-" : output.toString();
    }

    private static List<String> cases() {
        List<String> values = new ArrayList<>(List.of("", "cote", "coté", "côte", "côté", "COTE"));
        for (int count : new int[]{0, 1, 2, 7, 127, 253}) {
            String middle = "a".repeat(count);
            values.add("é" + middle + "è");
            values.add("è" + middle + "é");
            values.add("é" + middle + "e");
            values.add("e" + middle + "é");
        }
        Random random = new Random(RANDOM_SEED);
        for (int number = 0; number < RANDOM_CASES; number++) {
            int length = 1 + random.nextInt(SHORT_LIMIT);
            StringBuilder value = new StringBuilder();
            for (int position = 0; position < length; position++) {
                value.append(ALPHABET.charAt(random.nextInt(ALPHABET.length())));
            }
            values.add(value.toString());
        }
        return values;
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 0) {
            throw new IllegalArgumentException("No arguments expected");
        }
        var encoder = IndexCodes.getTextIndexCodes(new ColumnImpl.SortOrder(LCID, VARIANT, VERSION));
        if (encoder == null || !encoder.getClass().getSimpleName().equals("FrenchIndexCodes")) {
            throw new AssertionError("Exact French descriptor required");
        }
        for (char unit : ALPHABET.toCharArray()) {
            var handler = encoder.getCharHandler(unit);
            System.out.printf("weight\t%04x\t%s\t%d\t%d\t%d\t%d\t%d\t%s\t%s\t%s\t%s%n",
                    (int) unit, handler.getType().name(), handler.getNumCharUnits(), handler.getNumPrimaryUnits(),
                    handler.getExtraPlaceholder() & 0xff, handler.getExtraByteModifier() & 0xff,
                    handler.getCrazyFlag() & 0xff, hex(handler.getInlineBytes(unit)), hex(handler.getExtraBytes()),
                    hex(handler.getUnprintableBytes()), hex(encoder.getTertiaryWeights(unit)));
        }
        for (String value : cases()) {
            var output = new ByteUtil.ByteStream();
            encoder.writeNonNullIndexTextValue(value, output, true);
            System.out.printf("short\t%s\t%s%n", units(value), hex(output.toByteArray()));
        }
    }
}
