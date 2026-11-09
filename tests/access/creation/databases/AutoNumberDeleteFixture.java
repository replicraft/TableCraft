package com.healthmarketscience.jackcess.impl;

import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.Cursor;
import com.healthmarketscience.jackcess.CursorBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Index;
import com.healthmarketscience.jackcess.IndexBuilder;
import com.healthmarketscience.jackcess.PropertyMap;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Base64;
import java.util.HexFormat;
import java.util.List;
import java.util.Set;
import java.util.TimeZone;

/** Original public-writer fixture; inspection never requests a new AutoNumber. */
public final class AutoNumberDeleteFixture {
    private static final int GENERATED_ROWS = 260;
    private static final int MAX_IMAGE_BYTES = 8 * 1024 * 1024;
    private static final int APPEND_SCORE = -1234567;
    private static final Set<Integer> KEPT_IDS = Set.of(1, 3, 7, 64, 128, 259, 260);
    private static final List<String> TABLES = List.of("Numbered", "Loose");

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setTimeZone(TimeZone.getTimeZone("UTC"));
        database.setLinkResolver((source, token) -> {
            throw new AssertionError("External databases are not authorized");
        });
    }

    private static String hash(byte[] raw) throws Exception {
        return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(raw));
    }

    private static byte[] image(Path path) throws Exception {
        if (Files.size(path) > MAX_IMAGE_BYTES) {
            throw new IllegalArgumentException("Fixture exceeds bounded image size");
        }
        return Files.readAllBytes(path);
    }

    private static void generate(Path path, String profile) throws Exception {
        if (Files.exists(path) || !List.of("V2016", "V2019").contains(profile)) {
            throw new IllegalArgumentException("Expected fresh output and ACE16 profile");
        }

        try (Database database = new DatabaseBuilder(path)
                .setFileFormat(Database.FileFormat.valueOf(profile)).create()) {
            inert(database);
            for (String name : TABLES) {
                TableBuilder builder = new TableBuilder(name)
                    .addColumn(new ColumnBuilder("ID", DataType.LONG).setAutoNumber(true))
                    .addColumn(new ColumnBuilder("Score", DataType.LONG));
                if (name.equals("Numbered")) {
                    builder.addIndex(new IndexBuilder("Primary").addColumns("ID").setPrimaryKey());
                    builder.addIndex(new IndexBuilder("PrimaryAlias").addColumns("ID")
                        .setRequired().setUnique());
                }
                Table table = builder.toTable(database);
                for (int ordinal = 1; ordinal <= GENERATED_ROWS; ordinal++) {
                    Object[] row = table.addRow(Column.AUTO_NUMBER, ordinal * 10);
                    if (!Integer.valueOf(ordinal).equals(row[0])) {
                        throw new AssertionError("Unexpected construction AutoNumber");
                    }
                }

                List<Row> removed = new ArrayList<>();
                for (Row row : table) {
                    if (!KEPT_IDS.contains(row.getInt("ID"))) {
                        removed.add(row);
                    }
                }
                for (Row row : removed) {
                    table.deleteRow(row);
                }
            }
        }
    }

    private static void copyAction(Path source, Path output, String name, String selection)
            throws Exception {
        if (!TABLES.contains(name)) {
            throw new IllegalArgumentException("Unknown table");
        }
        String before = hash(image(source));
        Files.copy(source, output);
        try (Database database = new DatabaseBuilder(output).open()) {
            inert(database);
            Table table = database.getTable(name);
            if (selection.equals("append")) {
                Object[] inserted = table.addRow(Column.AUTO_NUMBER, APPEND_SCORE);
                System.out.printf("APPEND\t%s\t%s%n", name, inserted[0]);
            } else {
                Set<Integer> selected = switch (selection) {
                    case "first" -> Set.of(1);
                    case "highest" -> Set.of(260);
                    case "subset" -> Set.of(3, 128, 260);
                    case "all" -> KEPT_IDS;
                    default -> throw new IllegalArgumentException("Unknown selection");
                };
                List<Row> removed = new ArrayList<>();
                for (Row row : table) {
                    if (selected.contains(row.getInt("ID"))) {
                        removed.add(row);
                    }
                }
                if (removed.size() != selected.size()) {
                    throw new AssertionError("Selection is not complete");
                }
                for (Row row : removed) {
                    table.deleteRow(row);
                }
            }
        }
        if (!before.equals(hash(image(source)))) {
            throw new AssertionError("Source changed during copy action");
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
                JetFormat format = physical.getFormat();
                int base = Math.multiplyExact(physical.getTableDefPageNumber(), format.PAGE_SIZE);
                int longOffset = Math.addExact(base, format.OFFSET_NEXT_AUTO_NUMBER);
                int complexOffset = Math.addExact(base, format.OFFSET_NEXT_COMPLEX_AUTO_NUMBER);
                int rawLong = bytes.getInt(longOffset);
                int rawComplex = bytes.getInt(complexOffset);
                if (rawLong != physical.getLastLongAutoNumber()
                        || rawComplex != physical.getLastComplexTypeAutoNumber()) {
                    throw new AssertionError("Raw and getter counters disagree");
                }
                System.out.printf("TABLE\t%s\t%d\t%d\t%d\t%d\t%d\t%d\t%d\t%d%n",
                    name, table.getRowCount(), physical.getTableDefPageNumber(),
                    bytes.getInt(base + format.OFFSET_NUM_ROWS), rawLong, longOffset,
                    rawComplex, complexOffset, format.PAGE_SIZE);
                int indexCount = bytes.getInt(base + format.OFFSET_NUM_INDEXES);
                int columns = base + format.OFFSET_INDEX_DEF_BLOCK
                    + indexCount * format.SIZE_INDEX_DEFINITION;
                for (Column value : table.getColumns()) {
                    ColumnImpl column = (ColumnImpl) value;
                    if (column.getColumnIndex() != column.getColumnNumber()) {
                        throw new AssertionError("Unexpected fresh column ordering");
                    }
                    int start = columns + column.getColumnIndex() * format.SIZE_COLUMN_HEADER;
                    System.out.printf("COLUMN\t%s\t%s\t%s\t%s\t%d\t%d\t%d%n", name,
                        column.getName(), column.getType(), column.isAutoNumber(), start,
                        Byte.toUnsignedInt(raw[start + format.OFFSET_COLUMN_FLAGS]),
                        Byte.toUnsignedInt(raw[start + format.OFFSET_COLUMN_EXT_FLAGS]));
                    for (PropertyMap.Property property : column.getProperties()) {
                        String encoded = Base64.getEncoder().encodeToString(
                            String.valueOf(property.getValue()).getBytes(StandardCharsets.UTF_8));
                        System.out.printf("PROPERTY\t%s\t%s\t%s\t%s\t%s\t%s%n", name,
                            column.getName(), property.getName(), property.getType(),
                            property.isDdl(), encoded);
                    }
                }
                for (Row row : table) {
                    RowIdImpl location = (RowIdImpl) row.getId();
                    System.out.printf("ROW\t%s\t%d\t%d\t%d\t%d%n", name,
                        row.getInt("ID"), row.getInt("Score"), location.getPageNumber(),
                        location.getRowNumber());
                }
                for (Index index : table.getIndexes()) {
                    IndexImpl logical = (IndexImpl) index;
                    IndexData data = logical.getIndexData();
                    System.out.printf("INDEX\t%s\t%s\t%d\t%d\t%d\t%d\t%d%n", name,
                        index.getName(), data.getIndexDataNumber(), data.getRootPageNumber(),
                        data.getEntryCount(), logical.getUniqueEntryCount(),
                        logical.getUniqueEntryCountOffset());
                    Cursor cursor = CursorBuilder.createCursor(index);
                    Row row;
                    while ((row = cursor.getNextRow()) != null) {
                        System.out.printf("ORDER\t%s\t%s\t%d%n", name, index.getName(), row.getInt("ID"));
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
        } else if (args.length == 5 && args[0].equals("copy")) {
            copyAction(Path.of(args[1]), Path.of(args[2]), args[3], args[4]);
        } else {
            throw new IllegalArgumentException("Expected generate PATH PROFILE, oracle PATH, or copy SOURCE OUTPUT TABLE SELECTION");
        }
    }
}
