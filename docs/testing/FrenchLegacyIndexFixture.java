package com.healthmarketscience.jackcess.impl;

import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.Cursor;
import com.healthmarketscience.jackcess.CursorBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Index;
import com.healthmarketscience.jackcess.IndexBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.Arrays;
import java.util.Base64;
import java.util.HexFormat;
import java.util.List;
import java.util.TimeZone;

/** Original public-writer recipe and inert read-only encoded-key observations. */
public final class FrenchLegacyIndexFixture {
    private static final short FRENCH_LCID = 1036;
    private static final short GENERAL_LCID = 1033;
    private static final short LEGACY_VERSION = 0;
    private static final short VARIANT = 0;
    private static final int MAX_IMAGE_BYTES = 8 * 1024 * 1024;
    private static final List<String> TABLES = List.of("French", "GeneralLegacy");
    private static final String[] VALUES = {
        null, "", "cote", "coté", "côte", "côté", "COTE", "cote",
        "cote ", "co-te", "co'te", "co te", "co.te", "cote!",
        "é", "e\u0301", "è", "ê", "ë", "e", "E", "É",
        "œ", "oe", "æ", "ae", "ß", "ss", "Œ", "OE",
        "😀", "a\u0000b", "a".repeat(255), "a".repeat(254) + "é",
        "a".repeat(254) + "è", "à".repeat(255), "côté", "cote  "
    };

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setTimeZone(TimeZone.getTimeZone("UTC"));
        database.setLinkResolver((source, token) -> {
            throw new AssertionError("External database not authorized");
        });
    }

    private static String hash(byte[] bytes) throws Exception {
        return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
    }

    private static byte[] image(Path path) throws Exception {
        if (Files.size(path) > MAX_IMAGE_BYTES) {
            throw new IllegalArgumentException("Image exceeds fixture limit");
        }
        return Files.readAllBytes(path);
    }

    private static void generate(Path path, String profile) throws Exception {
        if (Files.exists(path) || !List.of("V2016", "V2019").contains(profile)) {
            throw new IllegalArgumentException("Expected fresh ACE16 output");
        }
        try (Database database = new DatabaseBuilder(path)
                .setFileFormat(Database.FileFormat.valueOf(profile)).create()) {
            inert(database);
            for (String name : TABLES) {
                short lcid = name.equals("French") ? FRENCH_LCID : GENERAL_LCID;
                ColumnImpl.SortOrder order = new ColumnImpl.SortOrder(lcid, VARIANT, LEGACY_VERSION);
                ColumnBuilder word = new ColumnBuilder("Word", DataType.TEXT).setLengthInUnits(255);
                word.setTextSortOrder(order);

                Table table = new TableBuilder(name)
                    .addColumn(new ColumnBuilder("ID", DataType.LONG))
                    .addColumn(word)
                    .addIndex(new IndexBuilder("Asc").addColumns("Word"))
                    .addIndex(new IndexBuilder("Twin").addColumns("Word"))
                    .addIndex(new IndexBuilder("Desc").addColumns(false, "Word"))
                    .toTable(database);
                for (int position = 0; position < VALUES.length; position++) {
                    table.addRow(position + 1, VALUES[position]);
                }
            }
        }
    }

    private static void oracle(Path path) throws Exception {
        byte[] raw = image(path);
        String before = hash(raw);
        ByteBuffer bytes = ByteBuffer.wrap(raw).order(ByteOrder.LITTLE_ENDIAN);
        System.out.printf("FILE\t%s\t%d\t%s%n", path.getFileName(), raw.length, before);
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            inert(database);
            for (String name : TABLES) {
                Table table = database.getTable(name);
                TableImpl physical = (TableImpl) table;
                ColumnImpl column = (ColumnImpl) table.getColumn("Word");
                JetFormat format = physical.getFormat();
                int base = Math.multiplyExact(physical.getTableDefPageNumber(), format.PAGE_SIZE);
                int physicalCount = bytes.getInt(base + format.OFFSET_NUM_INDEXES);
                int declaration = base + format.OFFSET_INDEX_DEF_BLOCK
                    + physicalCount * format.SIZE_INDEX_DEFINITION
                    + column.getColumnIndex() * format.SIZE_COLUMN_HEADER;
                int offset = declaration + format.OFFSET_COLUMN_SORT_ORDER;
                ColumnImpl.SortOrder stored = column.getTextSortOrder();
                byte[] descriptor = Arrays.copyOfRange(raw, offset, offset + format.SIZE_SORT_ORDER);
                System.out.printf("TABLE\t%s\t%d\t%d%n", name, table.getRowCount(), physical.getTableDefPageNumber());
                System.out.printf("SORT\t%s\t%d\t%d\t%d\t%d\t%s%n", name,
                    stored.getValue(), stored.getVariant(), stored.getVersion(), offset,
                    HexFormat.of().formatHex(descriptor));
                for (Row row : table) {
                    String value = row.getString("Word");
                    String encoded = value == null ? "NULL" : "UTF8:" + Base64.getEncoder()
                        .encodeToString(value.getBytes(StandardCharsets.UTF_8));
                    RowIdImpl location = (RowIdImpl) row.getId();
                    System.out.printf("ROW\t%s\t%d\t%d\t%d\t%s%n", name,
                        row.getInt("ID"), location.getPageNumber(), location.getRowNumber(), encoded);
                }
                for (Index index : table.getIndexes()) {
                    IndexData data = ((IndexImpl) index).getIndexData();
                    System.out.printf("INDEX\t%s\t%s\t%d\t%d\t%d\t%d%n", name,
                        index.getName(), data.getIndexDataNumber(), data.getRootPageNumber(),
                        data.getEntryCount(), data.getOwnedPageCount());
                    Cursor cursor = CursorBuilder.createCursor(index);
                    Row row;
                    while ((row = cursor.getNextRow()) != null) {
                        System.out.printf("ORDER\t%s\t%s\t%d%n", name, index.getName(), row.getInt("ID"));
                    }
                    IndexData.EntryCursor entries = data.cursor();
                    for (int position = 0; position < data.getEntryCount(); position++) {
                        IndexData.Entry entry = entries.getNextEntry();
                        if (!entry.isValid() || !entry.isLeafEntry()) {
                            throw new AssertionError("Expected complete index cursor");
                        }
                        RowIdImpl location = entry.getRowId();
                        System.out.printf("KEY\t%s\t%s\t%d\t%d\t%s%n", name,
                            index.getName(), location.getPageNumber(), location.getRowNumber(),
                            HexFormat.of().formatHex(entry.getEntryBytes()));
                    }
                }
            }
        }
        if (!before.equals(hash(image(path)))) {
            throw new AssertionError("Read-only oracle changed input");
        }
    }

    public static void main(String[] args) throws Exception {
        if (args.length == 3 && args[0].equals("generate")) {
            generate(Path.of(args[1]), args[2]);
        } else if (args.length == 2 && args[0].equals("oracle")) {
            oracle(Path.of(args[1]));
        } else {
            throw new IllegalArgumentException("Expected generate PATH PROFILE or oracle PATH");
        }
    }
}
