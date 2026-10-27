package com.healthmarketscience.jackcess.impl;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HexFormat;

/** Original bounded caller; no copied weights or comparison implementation. */
public final class ShortKeysOracle {
    private static final int HEX_DIGITS_PER_UNIT = 4;
    private static final int MAX_SHORT_UNITS = 255;

    public static void main(String[] args) throws Exception {
        if (args.length != 1) {
            throw new IllegalArgumentException("Expected UTF16BE-hex input file");
        }
        for (String line : Files.readAllLines(Path.of(args[0]))) {
            if (line.length() > MAX_SHORT_UNITS * HEX_DIGITS_PER_UNIT
                    || line.length() % HEX_DIGITS_PER_UNIT != 0) {
                throw new IllegalArgumentException("Expected bounded full UTF16 units");
            }
            StringBuilder value = new StringBuilder();
            for (int offset = 0; offset < line.length(); offset += HEX_DIGITS_PER_UNIT) {
                value.append((char) Integer.parseInt(line.substring(offset,
                    offset + HEX_DIGITS_PER_UNIT), 16));
            }
            for (short lcid : new short[]{1036, 1033}) {
                GeneralLegacyIndexCodes encoder = IndexCodes.getTextIndexCodes(
                    new ColumnImpl.SortOrder(lcid, (short) 0, (short) 0));
                if (encoder == null) {
                    throw new AssertionError("Missing exact reference profile");
                }
                ByteUtil.ByteStream bytes = new ByteUtil.ByteStream();
                encoder.writeNonNullIndexTextValue(value.toString(), bytes, true);
                System.out.printf("%d\t%s\t%s%n", lcid, line,
                    HexFormat.of().formatHex(bytes.toByteArray()));
            }
        }
    }
}
