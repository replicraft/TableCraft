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
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.HexFormat;
import java.util.List;
import java.util.TimeZone;

/** Original public-writer recipe; implementation getters are read-only oracles. */
public final class EncodedDeleteFixture {
    private static final String[] POSITIVE = {
        "TextLegacyLeaf", "TextModernLeaf", "BinaryLeaf",
        "TextLegacyNode", "TextModernNode", "BinaryNode"
    };
    private static final String[] NEGATIVE = {
        "TextUniqueOverflow", "MemoNegative", "TextCompositeNegative",
        "BinaryCompositeNegative", "TextIgnoredNegative", "BinaryIgnoredNegative"
    };

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setTimeZone(TimeZone.getTimeZone("UTC"));
        database.setLinkResolver((source, token) -> {
            throw new AssertionError("No external database is authorized");
        });
    }

    private static ColumnBuilder key(DataType type, int family) {
        ColumnBuilder builder = new ColumnBuilder("KeyValue", type);
        if (type == DataType.TEXT) {
            builder.setLengthInUnits(255);
        } else if (type == DataType.BINARY) {
            builder.setLength(255);
        }
        if (type == DataType.TEXT || type == DataType.MEMO) {
            builder.setTextSortOrder(family == 0
                ? ColumnImpl.GENERAL_LEGACY_SORT_ORDER : ColumnImpl.GENERAL_SORT_ORDER);
        }
        return builder;
    }

    private static TableBuilder base(String name, DataType type, int family) {
        return new TableBuilder(name)
            .addColumn(new ColumnBuilder("ID", DataType.LONG))
            .addColumn(key(type, family));
    }

    private static Object value(boolean binary, int index) {
        if (!binary) {
            String[] values = {null, "", "a", "A", "é", "e\u0301", "Straße",
                "Strasse", "a  ", "😀", "\u0000", "a".repeat(255), "カタカナ"};
            return values[index % values.length];
        }
        if (index % 9 == 0) {
            return null;
        }
        int[] lengths = {0, 0, 1, 7, 8, 9, 16, 17, 255};
        byte[] bytes = new byte[lengths[index % 9]];
        for (int position = 0; position < bytes.length; position++) {
            bytes[position] = (byte) (position * 31 + (index % 3 == 0 ? 255 : 128));
        }
        return bytes;
    }

    private static Table positive(Database database, String name, DataType type,
            int family, int rows) throws Exception {
        Table table = base(name, type, family)
            .addIndex(new IndexBuilder("Primary").addColumns("ID").setPrimaryKey())
            .addIndex(new IndexBuilder("Asc").addColumns("KeyValue"))
            .addIndex(new IndexBuilder("Twin").addColumns("KeyValue"))
            .addIndex(new IndexBuilder("Desc").addColumns(false, "KeyValue"))
            .toTable(database);
        for (int row = 0; row < rows; row++) {
            table.addRow(row + 1, value(type == DataType.BINARY, row * 7));
        }
        return table;
    }

    private static void generate(Path directory, String profile) throws Exception {
        if (!List.of("V2016", "V2019").contains(profile)) {
            throw new IllegalArgumentException("Only ACE16 profiles are admitted");
        }
        Path original = directory.resolve("Encoded-" + profile + "-before.accdb");
        if (Files.exists(original)) {
            throw new IllegalArgumentException("Original output already exists");
        }
        for (String state : List.of("delete-tie", "delete-null", "delete-all")) {
            if (Files.exists(directory.resolve("Encoded-" + profile + "-" + state + ".accdb"))) {
                throw new IllegalArgumentException("Reference output already exists");
            }
        }
        try (Database database = new DatabaseBuilder(original)
                .setFileFormat(Database.FileFormat.valueOf(profile)).create()) {
            inert(database);
            positive(database, POSITIVE[0], DataType.TEXT, 0, 24);
            positive(database, POSITIVE[1], DataType.TEXT, 1, 24);
            positive(database, POSITIVE[2], DataType.BINARY, 1, 24);
            positive(database, POSITIVE[3], DataType.TEXT, 0, 701);
            positive(database, POSITIVE[4], DataType.TEXT, 1, 701);
            positive(database, POSITIVE[5], DataType.BINARY, 1, 701);
            base(NEGATIVE[0], DataType.TEXT, 1)
                .addIndex(new IndexBuilder("UniqueOverflow").addColumns("KeyValue").setUnique())
                .toTable(database).addRow(1, "a".repeat(255));
            base(NEGATIVE[1], DataType.MEMO, 1)
                .addIndex(new IndexBuilder("Memo").addColumns("KeyValue"))
                .toTable(database).addRow(1, (Object) null);
            for (boolean binary : new boolean[]{false, true}) {
                String name = binary ? NEGATIVE[3] : NEGATIVE[2];
                Table table = base(name, binary ? DataType.BINARY : DataType.TEXT, 1)
                    .addIndex(new IndexBuilder("Composite").addColumns("KeyValue", "ID"))
                    .toTable(database);
                table.addRow(1, binary ? new byte[]{1} : "a");
                table.addRow(2, binary ? new byte[]{1} : "A");
                name = binary ? NEGATIVE[5] : NEGATIVE[4];
                base(name, binary ? DataType.BINARY : DataType.TEXT, 1)
                    .addIndex(new IndexBuilder("Ignored").addColumns("KeyValue").setIgnoreNulls())
                    .toTable(database).addRow(1, binary ? new byte[]{1} : "a");
            }
            byte[] payload = new byte[8192];
            for (int index = 0; index < payload.length; index++) {
                payload[index] = (byte) (index * 17 + 3);
            }
            new TableBuilder("PreservedPayloads")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("Notes", DataType.MEMO))
                .addColumn(new ColumnBuilder("Payload", DataType.OLE))
                .toTable(database).addRow(1, "Sentinel é中😀|".repeat(1024), payload);
        }
        System.out.println("GENERATED\t" + original.getFileName() + "\t" + Files.size(original));
        for (String state : List.of("delete-tie", "delete-null", "delete-all")) {
            Path copy = directory.resolve("Encoded-" + profile + "-" + state + ".accdb");
            Files.copy(original, copy);
            int total = 0;
            try (Database database = new DatabaseBuilder(copy).open()) {
                inert(database);
                for (String name : POSITIVE) {
                    Table table = database.getTable(name);
                    List<Row> selected = new ArrayList<>();
                    for (Row row : table) {
                        int id = row.getInt("ID");
                        if (state.equals("delete-all") || id == (state.equals("delete-null") ? 1 : 4)) {
                            selected.add(row);
                        }
                    }
                    for (Row row : selected) {
                        table.deleteRow(row);
                    }
                    total += selected.size();
                }
            }
            System.out.println("REFERENCE\t" + copy.getFileName() + "\t" + total);
        }
    }

    private static String hash(byte[] bytes) throws Exception {
        return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
    }

    private static String value(Object value) {
        if (value == null) {
            return "NULL";
        }
        if (value instanceof byte[] bytes) {
            return "B:" + HexFormat.of().formatHex(bytes);
        }
        StringBuilder units = new StringBuilder("T:");
        for (char unit : value.toString().toCharArray()) {
            units.append(String.format("%04x", (int) unit));
        }
        return units.toString();
    }

    private static void oracle(Path path) throws Exception {
        byte[] raw = Files.readAllBytes(path);
        System.out.printf("FILE\t%s\t%d\t%s%n", path.getFileName(), raw.length, hash(raw));
        List<String> names = new ArrayList<>(List.of(POSITIVE));
        names.addAll(List.of(NEGATIVE));
        names.add("PreservedPayloads");
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            inert(database);
            for (String name : names) {
                Table table = database.getTable(name);
                TableImpl physical = (TableImpl) table;
                System.out.printf("TABLE\t%s\t%d\t%d%n", name, table.getRowCount(), physical.getTableDefPageNumber());
                if (!name.equals("PreservedPayloads")) {
                    ColumnImpl column = (ColumnImpl) table.getColumn("KeyValue");
                    ColumnImpl.SortOrder order = column.getTextSortOrder();
                    System.out.printf("COLUMN\t%s\t%s\t%d\t%s%n", name, column.getType(), column.getLength(), order);
                }
                for (Row row : table) {
                    RowIdImpl location = (RowIdImpl) row.getId();
                    if (name.equals("PreservedPayloads")) {
                        System.out.printf("PAYLOAD\t%s\t%s\t%s%n", name,
                            hash(row.getString("Notes").getBytes(StandardCharsets.UTF_8)), hash(row.getBytes("Payload")));
                    } else {
                        System.out.printf("ROW\t%s\t%d\t%d\t%d\t%s%n", name,
                            row.getInt("ID"), location.getPageNumber(), location.getRowNumber(), value(row.get("KeyValue")));
                    }
                }
                for (Index index : table.getIndexes()) {
                    IndexImpl physicalIndex = (IndexImpl) index;
                    IndexData data = physicalIndex.getIndexData();
                    int root = data.getRootPageNumber();
                    int offset = root * 4096;
                    System.out.printf("INDEX\t%s\t%s\t%d\t%d\t%d\t%d\t%d\t%d\t%d\t%d%n", name,
                        index.getName(), data.getIndexDataNumber(), root,
                        Byte.toUnsignedInt(raw[offset]), Byte.toUnsignedInt(raw[offset + 26]),
                        data.getOwnedPageCount(), physicalIndex.getUniqueEntryCount(),
                        physicalIndex.getUniqueEntryCountOffset(), data.getEntryCount());
                    Cursor cursor = CursorBuilder.createCursor(index);
                    Row row;
                    while ((row = cursor.getNextRow()) != null) {
                        System.out.printf("ORDER\t%s\t%s\t%d%n", name, index.getName(), row.getInt("ID"));
                    }
                    IndexData.EntryCursor entries = data.cursor();
                    for (int position = 0; position < data.getEntryCount(); position++) {
                        IndexData.Entry entry = entries.getNextEntry();
                        if (!entry.isValid() || !entry.isLeafEntry()) {
                            throw new AssertionError("Expected complete leaf cursor");
                        }
                        RowIdImpl reference = entry.getRowId();
                        System.out.printf("KEY\t%s\t%s\t%d\t%d\t%s%n", name,
                            index.getName(), reference.getPageNumber(), reference.getRowNumber(),
                            HexFormat.of().formatHex(entry.getEntryBytes()));
                    }
                }
            }
            if (!database.getLinkedDatabases().isEmpty()) {
                throw new AssertionError("Unexpected external handles");
            }
        }
    }

    public static void main(String[] args) throws Exception {
        if (args.length == 3 && args[0].equals("generate")) {
            generate(Path.of(args[1]), args[2]);
        } else if (args.length == 2 && args[0].equals("oracle")) {
            oracle(Path.of(args[1]));
        } else {
            throw new IllegalArgumentException("Expected generate DIRECTORY PROFILE or oracle PATH");
        }
    }
}
