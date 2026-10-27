import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.Cursor;
import com.healthmarketscience.jackcess.CursorBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.IndexBuilder;
import com.healthmarketscience.jackcess.PropertyMap;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.TableBuilder;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import com.healthmarketscience.jackcess.impl.IndexData;
import com.healthmarketscience.jackcess.impl.RowIdImpl;
import com.healthmarketscience.jackcess.impl.TableImpl;
import com.healthmarketscience.jackcess.impl.UsageMap;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.Arrays;
import java.util.HexFormat;
import java.util.List;
import java.util.Set;
import java.util.TreeSet;

/** Repository-authored finite producer and inert read-only map/getter probe. */
public final class IndirectLongValueProbe {
    private static final String SELECTED = "IndirectLongValues";
    private static final String PADDING = "Padding";
    private static final String ID = "ID";
    private static final int LONG_UNITS = 8192;
    private static final int PADDING_ROWS = 700;
    private static final int PADDING_COLUMNS = 7;
    private static final int PADDING_UNITS = 255;
    private static final int MAX_MAP_MEMBERS = 32736;
    private static final long MAX_FILE_BYTES = 8L * 1024 * 1024;
    private static final List<String> LONG_COLUMNS = List.of("Memo", "OLE");

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setLinkResolver((source, token) -> {
            throw new AssertionError("No linked database is authorized");
        });
    }

    private static Object field(Object owner, String name) throws Exception {
        for (Class<?> type = owner.getClass(); type != null; type = type.getSuperclass()) {
            try {
                Field member = type.getDeclaredField(name);
                member.setAccessible(true);
                return member.get(owner);
            } catch (NoSuchFieldException missing) {
                // Continue only through the actual object's class hierarchy.
            }
        }

        throw new NoSuchFieldException(name);
    }

    private static String hash(byte[] bytes) throws Exception {
        return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
    }

    private static String text(int id) {
        return (id == 1 ? "Ā" : "ā").repeat(LONG_UNITS);
    }

    private static byte[] binary(int id) {
        byte[] bytes = new byte[256];
        for (int index = 0; index < bytes.length; index++) {
            bytes[index] = (byte) (id == 1 ? index : bytes.length - index - 1);
        }

        return bytes;
    }

    private static void bounded(Path path) throws Exception {
        if (Files.size(path) > MAX_FILE_BYTES) {
            throw new IllegalArgumentException("Frozen file ceiling exceeded");
        }
    }

    private static void createFirst(Path path, String profile) throws Exception {
        if (Files.exists(path) || !Set.of("V2016", "V2019").contains(profile)) {
            throw new IllegalArgumentException("Fresh approved profile required");
        }

        try (Database database = new DatabaseBuilder(path)
                .setFileFormat(Database.FileFormat.valueOf(profile)).setAutoSync(false).create()) {
            inert(database);
            TableImpl selected = (TableImpl) new TableBuilder(SELECTED)
                .addColumn(new ColumnBuilder(ID, DataType.LONG))
                .addColumn(new ColumnBuilder("Memo", DataType.MEMO).setCompressedUnicode(false))
                .addColumn(new ColumnBuilder("OLE", DataType.OLE))
                .addIndex(new IndexBuilder("PrimaryID").addColumns(ID).setPrimaryKey())
                .addIndex(new IndexBuilder("IDAsc").addColumns(ID))
                .addIndex(new IndexBuilder("IDTwin").addColumns(ID))
                .toTable(database);

            TableBuilder padding = new TableBuilder(PADDING)
                .addColumn(new ColumnBuilder(ID, DataType.LONG));
            for (int index = 0; index < PADDING_COLUMNS; index++) {
                padding.addColumn(new ColumnBuilder("Payload" + index, DataType.TEXT)
                    .setLengthInUnits(PADDING_UNITS).setCompressedUnicode(false));
            }
            padding.toTable(database);
            selected.addRow(1, text(1), binary(1));
            database.flush();
        }

        bounded(path);
    }

    private static void append(Path path, boolean paddingPhase) throws Exception {
        bounded(path);
        try (Database database = new DatabaseBuilder(path).setAutoSync(false).open()) {
            inert(database);
            TableImpl selected = (TableImpl) database.getTable(SELECTED);
            TableImpl padding = (TableImpl) database.getTable(PADDING);
            int expectedPadding = paddingPhase ? 0 : PADDING_ROWS;
            if (selected.getRowCount() != 1 || padding.getRowCount() != expectedPadding) {
                throw new IllegalArgumentException("Wrong frozen construction phase");
            }

            if (paddingPhase) {
                String value = "Ā".repeat(PADDING_UNITS);
                for (int id = 1; id <= PADDING_ROWS; id++) {
                    Object[] row = new Object[PADDING_COLUMNS + 1];
                    row[0] = id;
                    for (int index = 1; index < row.length; index++) {
                        row[index] = value;
                    }
                    padding.addRow(row);
                }
            } else {
                selected.addRow(2, text(2), binary(2));
            }
            database.flush();
        }

        bounded(path);
    }

    private static int mapNumber(UsageMap map, String method) throws Exception {
        Method getter = UsageMap.class.getDeclaredMethod(method);
        getter.setAccessible(true);
        return ((Number) getter.invoke(map)).intValue();
    }

    private static void map(String role, UsageMap map) throws Exception {
        System.out.printf("MAP\t%s\t%d\t%d\t%d\t%d%n", role,
            mapNumber(map, "getTablePageNumber"), mapNumber(map, "getRowStart"),
            mapNumber(map, "getRowEnd"), map.getPageCount());

        UsageMap.PageCursor cursor = map.cursor();
        cursor.beforeFirst();
        int count = 0;
        for (int page = cursor.getNextPage(); page >= 0; page = cursor.getNextPage()) {
            if (++count > MAX_MAP_MEMBERS) {
                throw new IllegalArgumentException("Map member capture ceiling exceeded");
            }
            System.out.printf("MEMBER\t%s\t%d%n", role, page);
        }
    }

    private static void properties(String owner, PropertyMap properties) throws Exception {
        for (PropertyMap.Property property : properties) {
            String value = String.valueOf(property.getValue());
            System.out.printf("PROPERTY\t%s\t%s\t%d\t%s\t%s%n", owner,
                property.getName(), Byte.toUnsignedInt(property.getType().getValue()),
                property.isDdl(), HexFormat.of().formatHex(value.getBytes(StandardCharsets.UTF_16LE)));
        }
    }

    private static void catalog(Database database) throws Exception {
        Set<String> users = database.getTableNames();
        TreeSet<String> names = new TreeSet<>(users);
        names.addAll(database.getSystemTableNames());
        if (names.size() > 64) {
            throw new IllegalArgumentException("Catalog capture ceiling exceeded");
        }
        for (String name : names) {
            TableImpl table = (TableImpl) (users.contains(name)
                ? database.getTable(name) : database.getSystemTable(name));
            String owner = "Table:" + table.getTableDefPageNumber();
            System.out.printf("TABLE\t%s\t%d\t%d\t%d\t%d%n", name,
                table.getTableDefPageNumber(), table.getRowCount(), table.getColumnCount(),
                table.getMaxColumnCount());
            map(owner + ":Owned", (UsageMap) field(table, "_ownedPages"));
            map(owner + ":Hint", (UsageMap) field(table, "_freeSpacePages"));

            for (Column column : table.getColumns()) {
                ColumnImpl nativeColumn = (ColumnImpl) column;
                System.out.printf("COLUMN\t%s\t%s\t%d\t%d\t%d\t%d\t%s\t%d%n",
                    name, column.getName(), column.getColumnIndex(), nativeColumn.getColumnNumber(),
                    Byte.toUnsignedInt(column.getType().getValue()), column.getLength(),
                    column.isCompressedUnicode(), nativeColumn.getVarLenTableIndex());
                if (column.getType() == DataType.MEMO || column.getType() == DataType.OLE) {
                    Object holder = field(column, "_lvalBufferH");
                    String role = owner + ":Column:" + nativeColumn.getColumnNumber();
                    if (holder == null) {
                        System.out.printf("MAP_STATUS\t%s\tAbsentHolder%n", role);
                    } else {
                        map(role + ":Owned", (UsageMap) field(holder, "_ownedPages"));
                        map(role + ":Hint", (UsageMap) field(holder, "_freeSpacePages"));
                    }
                }
            }

            for (var index : table.getIndexes()) {
                IndexData data = index.getIndexData();
                Method root = IndexData.class.getDeclaredMethod("getRootPageNumber");
                root.setAccessible(true);
                System.out.printf("INDEX\t%s\t%s\t%d\t%d\t%d%n", name,
                    index.getName(), data.getIndexDataNumber(), root.invoke(data), data.getEntryCount());
                map(owner + ":Index:" + index.getName(), (UsageMap) field(data, "_ownedPages"));
                if (name.equals(SELECTED)) {
                    Cursor cursor = CursorBuilder.createCursor(index);
                    Row row;
                    while ((row = cursor.getNextRow()) != null) {
                        System.out.printf("ENTRY\t%s\t%d%n", index.getName(), row.getInt(ID));
                    }
                }
            }
        }
    }

    private static void getter(Database database) throws Exception {
        TableImpl selected = (TableImpl) database.getTable(SELECTED);
        TableImpl padding = (TableImpl) database.getTable(PADDING);
        properties(SELECTED, selected.getProperties());
        for (Column column : selected.getColumns()) {
            properties(SELECTED + ":" + column.getName(), column.getProperties());
        }

        for (TableImpl table : List.of(selected, padding)) {
            int count = 0;
            for (Row row : table) {
                int id = row.getInt(ID);
                RowIdImpl location = (RowIdImpl) row.getId();
                ByteBuffer raw = TableImpl.positionAtRowData(table.createRowState(), location);
                byte[] bytes = new byte[raw.remaining()];
                raw.get(bytes);
                System.out.printf("ROW\t%s\t%d\t%d\t%d\t%d\t%s%n", table.getName(),
                    id, location.getPageNumber(), location.getRowNumber(), bytes.length, hash(bytes));

                if (table == selected) {
                    for (String name : LONG_COLUMNS) {
                        Object value = row.get(name);
                        byte[] actual = value instanceof String string
                            ? string.getBytes(StandardCharsets.UTF_16LE) : (byte[]) value;
                        byte[] expected = name.equals("Memo")
                            ? text(id).getBytes(StandardCharsets.UTF_16LE) : binary(id);
                        if (!Arrays.equals(actual, expected)) {
                            throw new AssertionError("Selected getter mismatch");
                        }
                        System.out.printf("VALUE\t%d\t%s\t%s%n", id, name,
                            HexFormat.of().formatHex(actual));
                    }
                    System.out.printf("RAWROW\t%d\t%s%n", id, HexFormat.of().formatHex(bytes));
                } else {
                    for (int index = 0; index < PADDING_COLUMNS; index++) {
                        if (!"Ā".repeat(PADDING_UNITS).equals(row.getString("Payload" + index))) {
                            throw new AssertionError("Padding getter mismatch");
                        }
                    }
                }
                count++;
            }
            if (count != table.getRowCount()) {
                throw new AssertionError("Complete getter count mismatch");
            }
        }
    }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length < 2) {
            throw new IllegalArgumentException("Explicit mode and path required");
        }

        String mode = arguments[0];
        Path path = Path.of(arguments[1]);
        if (mode.equals("create-first") && arguments.length == 3) {
            createFirst(path, arguments[2]);
        } else if (mode.equals("pad") && arguments.length == 2) {
            append(path, true);
        } else if (mode.equals("second") && arguments.length == 2) {
            append(path, false);
        } else if (mode.equals("inspect") && arguments.length == 2) {
            bounded(path);
            try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
                inert(database);
                getter(database);
                TableImpl selected = (TableImpl) database.getTable(SELECTED);
                map("GlobalFree", (UsageMap) field(selected.getPageChannel(), "_globalUsageMap"));
                catalog(database);
            }
        } else {
            throw new IllegalArgumentException("Operation outside finite recipe");
        }
    }
}
