package com.healthmarketscience.jackcess.impl;

import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.HexFormat;

/** Original inert getter/encoder oracle, without Access or SQL execution. */
public class TextEqualityCandidateOracle {
    private static byte[] key(GeneralLegacyIndexCodes encoder, String value) throws Exception {
        var output = new ByteUtil.ByteStream();
        encoder.writeNonNullIndexTextValue(value, output, true);
        return output.toByteArray();
    }

    private static String trimmed(String value) {
        int end = value.length();
        while (end > 0 && value.charAt(end - 1) == ' ') {
            end--;
        }
        return value.substring(0, end);
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 4) {
            throw new IllegalArgumentException("Expected input/index/General-version/literal-UTF8-hex");
        }
        int version = Integer.parseInt(args[2]);
        if (version != 0 && version != 1) {
            throw new IllegalArgumentException("Unknown General version");
        }
        GeneralLegacyIndexCodes encoder = version == 0
            ? GeneralLegacyIndexCodes.GEN_LEG_INSTANCE : GeneralIndexCodes.GEN_INSTANCE;
        String literal = new String(HexFormat.of().parseHex(args[3]), StandardCharsets.UTF_8);
        if (literal.length() > 255) {
            throw new IllegalArgumentException("Literal exceeds qualified Short Text envelope");
        }
        byte[] literalKey = key(encoder, trimmed(literal));
        int primary = 0;
        while (primary < literalKey.length && literalKey[primary] != 1) {
            primary++;
        }
        byte[] prefix = Arrays.copyOf(literalKey, Math.min(primary, 32));
        try (Database database = new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            Table table = database.getTable("TextIndexed");
            if (table.getColumn("Label").getType() != DataType.TEXT) {
                throw new IllegalArgumentException("Expected Short Text");
            }
            var cursor = table.newCursor().setIndex(table.getIndex(args[1])).toIndexCursor();
            Row row;
            while ((row = cursor.getNextRow()) != null) {
                String value = row.getString("Label");
                if (value == null) {
                    continue;
                }
                byte[] stored = key(encoder, value);
                String tagged = "I32:" + row.getInt("ID") + "\tT"
                    + HexFormat.of().formatHex(value.getBytes(StandardCharsets.UTF_8));
                boolean candidate = stored.length >= prefix.length
                    && Arrays.equals(prefix, Arrays.copyOf(stored, prefix.length));
                if (candidate) {
                    System.out.println("C\t" + tagged);
                }
                if (Arrays.equals(key(encoder, trimmed(value)), literalKey)) {
                    if (!candidate) {
                        throw new IllegalStateException("Lost an equal key");
                    }
                    System.out.println("E\t" + tagged);
                }
            }
        }
    }
}
