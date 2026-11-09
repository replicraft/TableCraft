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
import com.healthmarketscience.jackcess.impl.IndexImpl;
import com.healthmarketscience.jackcess.impl.RowIdImpl;
import com.healthmarketscience.jackcess.impl.TableImpl;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.HexFormat;
import java.util.List;

/** Original ACE16 candidate public-writer delete recipe and read-only metadata/getter oracle. */
public final class DeleteCandidateFixture {
    private static String hash(byte[] bytes) throws Exception {
        return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
    }

    private static void inert(Database database) {
        database.setTimeZone(java.util.TimeZone.getTimeZone("UTC"));
        database.setEvaluateExpressions(false);
        database.setLinkResolver((source, token) -> {
            throw new AssertionError("No linked targets are authorized");
        });
    }

    private static void generate(Path directory, String profile) throws Exception {
        if (!List.of("V2016", "V2019").contains(profile)) {
            throw new IllegalArgumentException("ACE16 profiles only");
        }
        Path original = directory.resolve("Candidate-" + profile + "-before.accdb");
        if (Files.exists(original)) {
            throw new IllegalArgumentException("Original output must be fresh");
        }
        try (Database database = new DatabaseBuilder(original)
                .setFileFormat(Database.FileFormat.valueOf(profile)).create()) {
            inert(database);
            Table indexed = new TableBuilder("Indexed")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("GroupKey", DataType.LONG))
                .addColumn(new ColumnBuilder("Part", DataType.INT))
                .addColumn(new ColumnBuilder("Score", DataType.LONG))
                .addColumn(new ColumnBuilder("Memo", DataType.MEMO))
                .addColumn(new ColumnBuilder("Payload", DataType.OLE))
                .addIndex(new IndexBuilder("PrimaryKey").addColumns("ID").setPrimaryKey())
                .addIndex(new IndexBuilder("GroupAsc").addColumns("GroupKey"))
                .addIndex(new IndexBuilder("GroupTwin").addColumns("GroupKey"))
                .addIndex(new IndexBuilder("GroupDesc").addColumns(false, "GroupKey"))
                .addIndex(new IndexBuilder("Compound").addColumns("Part", "GroupKey"))
                .addIndex(new IndexBuilder("UniqueScore").addColumns("Score").setUnique())
                .toTable(database);
            Integer[] groups = {10, 10, null, null, -1, 20};
            for (int index = 0; index < groups.length; index++) {
                int id = index + 1;
                byte[] payload = new byte[8192];
                for (int position = 0; position < payload.length; position++) {
                    payload[position] = (byte) (id * 17 + position * 3);
                }
                indexed.addRow(id, groups[index], (short) (index % 2 + 1), id * 100,
                    "row=" + id + ":中😀:" + "m".repeat(4096), payload);
            }
            TableBuilder codecBuilder = new TableBuilder("CodecKeys")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("ByteKey", DataType.BYTE))
                .addColumn(new ColumnBuilder("ShortKey", DataType.INT))
                .addColumn(new ColumnBuilder("LongKey", DataType.LONG))
                .addColumn(new ColumnBuilder("BigKey", DataType.BIG_INT))
                .addColumn(new ColumnBuilder("MoneyKey", DataType.MONEY))
                .addColumn(new ColumnBuilder("GuidKey", DataType.GUID))
                .addColumn(new ColumnBuilder("DecimalKey", DataType.NUMERIC)
                    .setPrecision(28).setScale(4))
                .addColumn(new ColumnBuilder("BooleanKey", DataType.BOOLEAN))
                .addIndex(new IndexBuilder("PrimaryKey").addColumns("ID").setPrimaryKey());
            for (String key : List.of("ByteKey", "ShortKey", "LongKey", "BigKey",
                    "MoneyKey", "GuidKey", "DecimalKey", "BooleanKey")) {
                codecBuilder.addIndex(new IndexBuilder(key + "Asc").addColumns(key));
                codecBuilder.addIndex(new IndexBuilder(key + "Desc").addColumns(false, key));
            }
            codecBuilder.addIndex(new IndexBuilder("MixedTuple")
                .addColumns("MoneyKey", "BigKey", "GuidKey"));
            Table codecs = codecBuilder.toTable(database);
            for (int id = 1; id <= 6; id++) {
                boolean missing = id == 3;
                long integer = id == 1 ? Long.MIN_VALUE : id == 6 ? Long.MAX_VALUE : id - 4;
                java.math.BigDecimal fixed = java.math.BigDecimal.valueOf(integer, 4);
                codecs.addRow(id, missing ? null : (byte) (id * 31),
                    missing ? null : (short) (id * 100 - 350),
                    missing ? null : id * 100000 - 350000,
                    missing ? null : integer, missing ? null : fixed,
                    missing ? null : String.format("{10203040-5060-7080-90A0-%012d}", id),
                    missing ? null : fixed, id % 2 == 0);
            }
            TableBuilder floatingBuilder = new TableBuilder("FloatingKeys")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("SingleKey", DataType.FLOAT))
                .addColumn(new ColumnBuilder("DoubleKey", DataType.DOUBLE))
                .addColumn(new ColumnBuilder("DateKey", DataType.SHORT_DATE_TIME));
            for (String key : List.of("SingleKey", "DoubleKey", "DateKey")) {
                floatingBuilder.addIndex(new IndexBuilder(key + "Asc").addColumns(key));
                floatingBuilder.addIndex(new IndexBuilder(key + "Desc").addColumns(false, key));
            }
            Table floating = floatingBuilder.toTable(database);
            double[] numbers = {-0.0, 0.0, 0.0, -1.5, 1.5, 1.5};
            for (int id = 1; id <= 6; id++) {
                boolean missing = id == 3;
                floating.addRow(id, missing ? null : (float) numbers[id - 1],
                    missing ? null : numbers[id - 1],
                    missing ? null : java.time.LocalDateTime.of(2020, 1, id, 12, 0));
            }
            Table unindexed = new TableBuilder("Unindexed")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("Score", DataType.LONG))
                .toTable(database);
            for (int id = 1; id <= 3; id++) {
                unindexed.addRow(id, id * 10);
            }
            new TableBuilder("Sentinel")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("TextValue", DataType.TEXT).setLengthInUnits(32))
                .toTable(database).addRow(91, "unchanged sentinel 中");
        }

        for (String operation : List.of("delete-one", "delete-null", "delete-all")) {
            Path copy = directory.resolve("Candidate-" + profile + "-" + operation + ".accdb");
            Files.copy(original, copy);
            try (Database database = new DatabaseBuilder(copy).open()) {
                inert(database);
                for (String name : List.of("Indexed", "CodecKeys", "FloatingKeys", "Unindexed")) {
                    Table table = database.getTable(name);
                    List<Row> selected = new ArrayList<>();
                    for (Row row : table) {
                        int id = row.getInt("ID");
                        if (operation.equals("delete-all")
                                || id == (operation.equals("delete-null") ? 3 : 2)) {
                            selected.add(row);
                        }
                    }
                    for (Row row : selected) {
                        table.deleteRow(row);
                    }
                }
            }
        }
    }

    private static String scalar(Object value) {
        return value == null ? "NULL" : value.toString();
    }

    private static void oracle(Path path) throws Exception {
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            inert(database);
            for (String name : List.of("Indexed", "CodecKeys", "FloatingKeys", "Unindexed", "Sentinel")) {
                Table table = database.getTable(name);
                TableImpl nativeTable = (TableImpl) table;
                var format = nativeTable.getFormat();
                System.out.printf("TABLE\t%s\t%d\t%d\t%d\t%d\t%d\t%d\t%d%n", name,
                    table.getRowCount(), nativeTable.getTableDefPageNumber(), format.PAGE_SIZE,
                    format.OFFSET_NUM_ROWS, format.OFFSET_ROW_START,
                    format.OFFSET_INDEX_ENTRY_MASK, format.SIZE_INDEX_ENTRY_MASK);
                for (var column : table.getColumns()) {
                    System.out.printf("COLUMN\t%s\t%s\t%s\t%d%n", name,
                        column.getName(), column.getType(), column.getColumnIndex());
                }
                for (Row row : table) {
                    RowIdImpl location = (RowIdImpl) row.getId();
                    int id = row.getInt("ID");
                    System.out.printf("LOCATION\t%s\t%d\t%d\t%d%n", name, id,
                        location.getPageNumber(), location.getRowNumber());
                    if (name.equals("Indexed")) {
                        byte[] memo = row.getString("Memo").getBytes(StandardCharsets.UTF_8);
                        byte[] payload = row.getBytes("Payload");
                        System.out.printf("ROW\t%s\t%d\t%s\t%s\t%s\t%d\t%s\t%d\t%s%n",
                            name, id, scalar(row.get("GroupKey")), scalar(row.get("Part")),
                            scalar(row.get("Score")), memo.length, hash(memo), payload.length, hash(payload));
                    } else if (name.equals("FloatingKeys")) {
                        Object single = row.get("SingleKey");
                        Object doubled = row.get("DoubleKey");
                        Object date = row.get("DateKey");
                        System.out.printf("ROW\t%s\t%d\t%s\t%s\t%s%n", name, id,
                            single == null ? "NULL" : Integer.toUnsignedString(
                                Float.floatToRawIntBits(((Number) single).floatValue())),
                            doubled == null ? "NULL" : Long.toUnsignedString(
                                Double.doubleToRawLongBits(((Number) doubled).doubleValue())),
                            date == null ? "NULL" : Long.toUnsignedString(
                                Double.doubleToRawLongBits(
                                    ((com.healthmarketscience.jackcess.impl.ColumnImpl)
                                        table.getColumn("DateKey")).toDateDouble(date))));
                    } else if (name.equals("CodecKeys")) {
                        List<String> values = new ArrayList<>();
                        for (String key : List.of("ByteKey", "ShortKey", "LongKey", "BigKey",
                                "MoneyKey", "GuidKey", "DecimalKey", "BooleanKey")) {
                            values.add(scalar(row.get(key)));
                        }
                        System.out.printf("ROW\t%s\t%d\t%s%n", name, id,
                            String.join("\t", values));
                    } else if (name.equals("Unindexed")) {
                        System.out.printf("ROW\t%s\t%d\t%s%n", name, id, scalar(row.get("Score")));
                    } else {
                        System.out.printf("ROW\t%s\t%d\t%s%n", name, id,
                            hash(row.getString("TextValue").getBytes(StandardCharsets.UTF_8)));
                    }
                }
                for (Index index : table.getIndexes()) {
                    IndexImpl nativeIndex = (IndexImpl) index;
                    var data = nativeIndex.getIndexData();
                    System.out.printf("INDEX\t%s\t%s\t%d\t%d\t%d\t%d\t%d\t%s\t%s%n",
                        name, index.getName(), data.getIndexDataNumber(),
                        nativeIndex.getUniqueEntryCount(), nativeIndex.getUniqueEntryCountOffset(),
                        data.getEntryCount(), data.getOwnedPageCount(), index.isUnique(),
                        index.shouldIgnoreNulls());
                    for (var column : index.getColumns()) {
                        System.out.printf("INDEX_COLUMN\t%s\t%s\t%s\t%s%n", name,
                            index.getName(), column.getName(), column.isAscending());
                    }
                    Cursor cursor = CursorBuilder.createCursor(index);
                    Row row;
                    while ((row = cursor.getNextRow()) != null) {
                        System.out.printf("ENTRY\t%s\t%s\t%d%n", name, index.getName(), row.getInt("ID"));
                    }
                }
            }
            if (!database.getLinkedDatabases().isEmpty()) {
                throw new AssertionError("Unexpected linked handles");
            }
        }
    }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length == 3 && arguments[0].equals("generate")) {
            generate(Path.of(arguments[1]), arguments[2]);
        } else if (arguments.length == 2 && arguments[0].equals("oracle")) {
            oracle(Path.of(arguments[1]));
        } else {
            throw new IllegalArgumentException("Expected generate DIRECTORY PROFILE or oracle PATH");
        }
    }
}
