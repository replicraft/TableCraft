import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.Cursor;
import com.healthmarketscience.jackcess.CursorBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.IndexBuilder;
import com.healthmarketscience.jackcess.PropertyMap;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import com.healthmarketscience.jackcess.impl.IndexData;
import com.healthmarketscience.jackcess.impl.RowIdImpl;
import com.healthmarketscience.jackcess.impl.TableImpl;
import java.lang.reflect.Method;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.HexFormat;
import java.util.List;
import java.util.Locale;
import java.util.TimeZone;
import java.util.UUID;

/** Original inert same-slot inputs and fresh public updateRow reference copies. */
public final class SlotUpdateFixture {
    private static final int TEXT_UNITS = 255;
    private static final int BINARY_BYTES = 255;
    private static final int INDEXED_ROWS = 500;
    private static final short FRENCH_LCID = 1036;
    private static final short SORT_VARIANT = 0;
    private static final byte LEGACY_VERSION = 0;

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setEnforceForeignKeys(true);
        database.setTimeZone(TimeZone.getTimeZone("UTC"));
        database.setLinkResolver((source, token) -> {
            throw new AssertionError("No linked database is authorized");
        });
    }

    private static String hash(byte[] bytes) throws Exception {
        return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
    }

    private static ColumnBuilder text(String name, boolean compressed, int family) {
        ColumnBuilder column = new ColumnBuilder(name, DataType.TEXT)
            .setLengthInUnits(TEXT_UNITS).setCompressedUnicode(compressed);
        column.setTextSortOrder(family == 0
            ? ColumnImpl.GENERAL_LEGACY_SORT_ORDER : ColumnImpl.GENERAL_SORT_ORDER);
        return column;
    }

    private static TableBuilder base(String name) {
        return new TableBuilder(name)
            .addColumn(new ColumnBuilder("ID", DataType.LONG))
            .addIndex(new IndexBuilder("PrimaryID").addColumns("ID").setPrimaryKey());
    }

    private static byte[] bytes(int length, int seed) {
        byte[] result = new byte[length];
        for (int index = 0; index < length; index++) {
            result[index] = (byte) (seed + index * 31);
        }
        return result;
    }

    private static void storageText(Database database, String name, boolean compressed, int family)
            throws Exception {
        Table table = base(name).addColumn(text("Label", compressed, family)).toTable(database);
        String[] values = {
            null, "", "ĀāĂ", "ABCD", "éàß", "ABC", "Aα", "\u0000", "A\u0000B",
            "\uFEFF", "\uFEFFAB", "\u00FF", "😀", "a".repeat(TEXT_UNITS),
            "Ā".repeat(TEXT_UNITS), "é".repeat(TEXT_UNITS), "日本語"
        };
        for (int index = 0; index < values.length; index++) {
            table.addRow(index + 1, values[index]);
        }
    }

    private static void indexed(Database database, String name, int family) throws Exception {
        Table table = base(name)
            .addColumn(text("Label", true, family))
            .addColumn(new ColumnBuilder("Bytes", DataType.BINARY).setLength(BINARY_BYTES))
            .addColumn(new ColumnBuilder("N", DataType.LONG))
            .addColumn(new ColumnBuilder("Token", DataType.GUID))
            .addIndex(new IndexBuilder("TextAsc").addColumns("Label"))
            .addIndex(new IndexBuilder("TextAlias").addColumns("Label"))
            .addIndex(new IndexBuilder("TextDesc").addColumns(false, "Label"))
            .addIndex(new IndexBuilder("BytesAsc").addColumns("Bytes"))
            .addIndex(new IndexBuilder("BytesAlias").addColumns("Bytes"))
            .addIndex(new IndexBuilder("BytesDesc").addColumns(false, "Bytes"))
            .addIndex(new IndexBuilder("TextIDUnique").addColumns("Label", "ID").setUnique())
            .addIndex(new IndexBuilder("Mixed").addColumns("Label", "Bytes", "N", "Token"))
            .toTable(database);
        String[] labels = {null, "", "Alpha", "beta", "cafe", "café", "a ",
            "zeta", "Straße", "Aα", "ĀāĂ", "ABCD"};
        int[] lengths = {0, 0, 7, 8, 9, 16};
        for (int index = 0; index < INDEXED_ROWS; index++) {
            byte[] binary = index % lengths.length == 0
                ? null : bytes(lengths[index % lengths.length], index % 17);
            UUID token = UUID.nameUUIDFromBytes(
                ("TableCraft slot original/" + name + "/" + index)
                    .getBytes(StandardCharsets.UTF_8));
            table.addRow(index + 1, labels[index % labels.length], binary,
                index % 37 - 18, "{" + token + "}");
        }
    }

    private static void unique(Database database, String name, int family) throws Exception {
        Table table = base(name)
            .addColumn(text("Label", true, family))
            .addColumn(new ColumnBuilder("Bytes", DataType.BINARY).setLength(BINARY_BYTES))
            .addIndex(new IndexBuilder("TextUnique").addColumns("Label").setUnique())
            .addIndex(new IndexBuilder("TextUniqueAlias").addColumns("Label").setUnique())
            .addIndex(new IndexBuilder("BytesUnique").addColumns("Bytes").setUnique())
            .addIndex(new IndexBuilder("BytesUniqueAlias").addColumns("Bytes").setUnique())
            .toTable(database);
        for (int identity = 1; identity <= 16; identity++) {
            byte[] binary = ByteBuffer.allocate(Long.BYTES).putLong(identity).array();
            table.addRow(identity, String.format(Locale.ROOT, "uniq-%04d", identity), binary);
        }
    }

    private static void binaryStorage(Database database) throws Exception {
        Table table = base("BinaryStorage")
            .addColumn(new ColumnBuilder("Bytes", DataType.BINARY).setLength(BINARY_BYTES))
            .toTable(database);
        table.addRow(1, null);
        int[] lengths = {0, 1, 7, 8, 9, 16, 17, 254, 255};
        for (int index = 0; index < lengths.length; index++) {
            table.addRow(index + 2, bytes(lengths[index], 128));
        }
    }

    private static void properties(Database database) throws Exception {
        ColumnBuilder required = text("Label", true, 0)
            .putProperty("Required", true).putProperty("AllowZeroLength", true);
        Table table = base("RequiredText").addColumn(required).toTable(database);
        table.addRow(1, "");
        table.addRow(2, "ĀāĂ");

        ColumnBuilder nonempty = text("Label", true, 0)
            .putProperty("Required", false).putProperty("AllowZeroLength", false);
        table = base("NoEmptyText").addColumn(nonempty).toTable(database);
        table.addRow(1, null);
        table.addRow(2, "ĀāĂ");

        ColumnBuilder limited = text("Label", true, 0)
            .putProperty("ValidationRule", "Len([Label]) <= 3")
            .putProperty("AllowZeroLength", true);
        table = base("LenLimited").addColumn(limited).toTable(database);
        table.addRow(1, "ĀāĂ");
        table.addRow(2, "XYZ");

        ColumnBuilder comparison = text("Label", true, 0)
            .putProperty("ValidationRule", "StrComp([Label],\"z\",0) < 0");
        table = base("StrComp0").addColumn(comparison).toTable(database);
        table.addRow(1, "A");
        table.addRow(2, "B");
    }

    private static void blockers(Database database) throws Exception {
        ColumnBuilder french = text("Label", true, 0);
        french.setTextSortOrder(new ColumnImpl.SortOrder(
            FRENCH_LCID, SORT_VARIANT, LEGACY_VERSION));
        Table table = base("FrenchBlocked").addColumn(french)
            .addIndex(new IndexBuilder("French").addColumns("Label")).toTable(database);
        table.addRow(1, "abc");
        table.addRow(2, "def");

        table = base("IgnoredText").addColumn(text("Label", true, 0))
            .addIndex(new IndexBuilder("Ignored").addColumns("Label").setIgnoreNulls())
            .toTable(database);
        table.addRow(1, null);
        table.addRow(2, "abc");
        table = base("IgnoredBinary")
            .addColumn(new ColumnBuilder("Bytes", DataType.BINARY).setLength(BINARY_BYTES))
            .addIndex(new IndexBuilder("Ignored").addColumns("Bytes").setIgnoreNulls())
            .toTable(database);
        table.addRow(1, null);
        table.addRow(2, bytes(7, 128));
    }

    private static void generate(Path path, String profile) throws Exception {
        if (Files.exists(path) || !List.of("V2016", "V2019").contains(profile)) {
            throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
        }
        try (Database database = new DatabaseBuilder(path)
                .setFileFormat(Database.FileFormat.valueOf(profile)).create()) {
            inert(database);
            storageText(database, "CompactLegacy", true, 0);
            storageText(database, "RawLegacy", false, 0);
            storageText(database, "CompactModern", true, 1);
            storageText(database, "RawModern", false, 1);
            indexed(database, "IndexedLegacy", 0);
            indexed(database, "IndexedModern", 1);
            unique(database, "UniqueLegacy", 0);
            unique(database, "UniqueModern", 1);
            binaryStorage(database);
            properties(database);
            blockers(database);
            Table sentinel = base("Sentinel")
                .addColumn(new ColumnBuilder("Memo", DataType.MEMO))
                .addColumn(new ColumnBuilder("OLE", DataType.OLE)).toTable(database);
            sentinel.addRow(91, "preserved 中😀 ".repeat(700), bytes(12 * 1024, 71));
        }
    }

    private static int root(IndexData data) throws Exception {
        Method method = IndexData.class.getDeclaredMethod("getRootPageNumber");
        method.setAccessible(true);
        return (Integer) method.invoke(data);
    }

    private static String value(Object value) throws Exception {
        if (value == null) {
            return "NULL";
        }
        if (value instanceof String text) {
            return "String16:" + HexFormat.of().formatHex(text.getBytes(StandardCharsets.UTF_16LE));
        }
        if (value instanceof byte[] binary) {
            return "Binary:" + HexFormat.of().formatHex(binary);
        }
        return value.getClass().getSimpleName() + ":" + value;
    }

    private static void properties(String table, String owner, PropertyMap map) throws Exception {
        for (PropertyMap.Property property : map) {
            System.out.printf("PROPERTY\t%s\t%s\t%s\t%d\t%s\t%s%n", table, owner,
                property.getName(), Byte.toUnsignedInt(property.getType().getValue()),
                property.isDdl(), value(property.getValue()));
        }
    }

    private static void oracle(Path path) throws Exception {
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            inert(database);
            for (String name : database.getTableNames()) {
                TableImpl table = (TableImpl) database.getTable(name);
                System.out.printf("TABLE\t%s\t%d\t%d\t%d\t%d%n", name, table.getRowCount(),
                    table.getTableDefPageNumber(), table.getColumnCount(), table.getMaxColumnCount());
                properties(name, "$table", table.getProperties());
                for (var column : table.getColumns()) {
                    ColumnImpl nativeColumn = (ColumnImpl) column;
                    var sort = nativeColumn.getTextSortOrder();
                    int lcid = sort == null ? 0 : Short.toUnsignedInt(sort.getValue());
                    int variant = sort == null ? 0 : Short.toUnsignedInt(sort.getVariant());
                    int version = sort == null ? 0 : Short.toUnsignedInt(sort.getVersion());
                    System.out.printf("COLUMN\t%s\t%s\t%d\t%d\t%d\t%d\t%s\t%d\t%d\t%d\t%d%n",
                        name, column.getName(), column.getColumnIndex(), nativeColumn.getColumnNumber(),
                        Byte.toUnsignedInt(column.getType().getValue()), column.getLength(),
                        column.isCompressedUnicode(), nativeColumn.getVarLenTableIndex(), lcid, variant, version);
                    properties(name, column.getName(), column.getProperties());
                }
                for (Row row : table) {
                    RowIdImpl location = (RowIdImpl) row.getId();
                    List<String> fields = new ArrayList<>();
                    fields.add("ROW");
                    fields.add(name);
                    fields.add(row.getInt("ID").toString());
                    fields.add(Integer.toString(location.getPageNumber()));
                    fields.add(Integer.toString(location.getRowNumber()));
                    for (var column : table.getColumns()) {
                        Object stored = row.get(column.getName());
                        if (column.getType() == DataType.MEMO && stored != null) {
                            fields.add("MemoSHA:" + hash(stored.toString().getBytes(StandardCharsets.UTF_8)));
                        } else if (column.getType() == DataType.OLE && stored != null) {
                            byte[] binary = (byte[]) stored;
                            fields.add("OLESHA:" + binary.length + ":" + hash(binary));
                        } else {
                            fields.add(value(stored));
                        }
                    }
                    System.out.println(String.join("\t", fields));
                    ByteBuffer buffer = TableImpl.positionAtRowData(table.createRowState(), location);
                    byte[] raw = new byte[buffer.remaining()];
                    buffer.get(raw);
                    System.out.printf("RAWROW\t%s\t%d\t%s%n", name, row.getInt("ID"),
                        HexFormat.of().formatHex(raw));
                }
                for (var index : table.getIndexes()) {
                    IndexData data = index.getIndexData();
                    System.out.printf("INDEX\t%s\t%s\t%d\t%d\t%d\t%d\t%s\t%s\t%s\t%s%n",
                        name, index.getName(), data.getIndexDataNumber(), root(data), data.getEntryCount(),
                        data.getOwnedPageCount(), index.isPrimaryKey(), index.isUnique(),
                        index.isRequired(), index.shouldIgnoreNulls());
                    for (var column : index.getColumns()) {
                        System.out.printf("INDEX_COLUMN\t%s\t%s\t%s\t%s%n", name, index.getName(),
                            column.getName(), column.isAscending());
                    }
                    Cursor cursor = CursorBuilder.createCursor(index);
                    Row row;
                    while ((row = cursor.getNextRow()) != null) {
                        System.out.printf("ENTRY\t%s\t%s\t%d%n", name, index.getName(), row.getInt("ID"));
                    }
                }
            }
        }
    }

    private static void update(Path source, Path output, String tableName, int identity,
            String column, String replacement) throws Exception {
        if (Files.exists(output)) {
            throw new IllegalArgumentException("Fresh update output required");
        }
        Object value;
        if (replacement.equals("null")) {
            value = null;
        } else if (replacement.startsWith("text16:")) {
            byte[] encoded = HexFormat.of().parseHex(replacement.substring("text16:".length()));
            if (encoded.length % Character.BYTES != 0) {
                throw new IllegalArgumentException("Text requires complete UTF16 units");
            }
            value = new String(encoded, StandardCharsets.UTF_16LE);
        } else if (replacement.startsWith("binary:")) {
            value = HexFormat.of().parseHex(replacement.substring("binary:".length()));
        } else {
            throw new IllegalArgumentException("Expected null, text16:HEX or binary:HEX");
        }
        Files.copy(source, output);
        try (Database database = new DatabaseBuilder(output).open()) {
            inert(database);
            Table table = database.getTable(tableName);
            Row selected = null;
            for (Row row : table) {
                if (row.getInt("ID") == identity) {
                    selected = row;
                    break;
                }
            }
            if (selected == null) {
                throw new IllegalArgumentException("Original ID absent");
            }
            selected.put(column, value);
            table.updateRow(selected);
            System.out.printf("UPDATED\t%s\t%d\t%s%n", tableName, identity, column);
        }
    }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length == 3 && arguments[0].equals("generate")) {
            generate(Path.of(arguments[1]), arguments[2]);
        } else if (arguments.length == 2 && arguments[0].equals("oracle")) {
            oracle(Path.of(arguments[1]));
        } else if (arguments.length == 7 && arguments[0].equals("update")) {
            update(Path.of(arguments[1]), Path.of(arguments[2]), arguments[3],
                Integer.parseInt(arguments[4]), arguments[5], arguments[6]);
        } else {
            throw new IllegalArgumentException("Expected generate PATH PROFILE, oracle PATH, or update SOURCE OUTPUT TABLE ID COLUMN VALUE");
        }
    }
}
