import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.Cursor;
import com.healthmarketscience.jackcess.CursorBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.DateTimeType;
import com.healthmarketscience.jackcess.Index;
import com.healthmarketscience.jackcess.IndexBuilder;
import com.healthmarketscience.jackcess.PropertyMap;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import com.healthmarketscience.jackcess.impl.IndexData;
import com.healthmarketscience.jackcess.impl.RowIdImpl;
import com.healthmarketscience.jackcess.impl.TableImpl;
import com.healthmarketscience.jackcess.impl.UsageMap;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.ByteBuffer;
import java.nio.channels.FileChannel;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.HexFormat;
import java.util.List;
import java.util.Set;
import java.util.TimeZone;
import java.util.TreeSet;

/** One genuine empty public original; fixed single/batch reference requests. */
public final class EmptyOwnedInsertFixture {
    private static final long MAX_FILE_BYTES = 8L * 1024 * 1024;
    private static final int MAX_MAP_MEMBERS = 32736;
    private static final int MAX_TABLES = 64;
    private static final int MAX_COLUMNS = 256;
    private static final int MAX_ROWS = 4096;

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setEnforceForeignKeys(true);
        database.setTimeZone(TimeZone.getTimeZone("UTC"));
        database.setDateTimeType(DateTimeType.LOCAL_DATE_TIME);
        database.setLinkResolver((source, token) -> {
            throw new AssertionError("No linked database is authorized");
        });
    }

    private static String hex(String text) {
        return HexFormat.of().formatHex(text.getBytes(StandardCharsets.UTF_16LE));
    }

    private static Object field(Object owner, String name) throws Exception {
        for (Class<?> type = owner.getClass(); type != null; type = type.getSuperclass()) {
            try {
                Field field = type.getDeclaredField(name);
                field.setAccessible(true);
                return field.get(owner);
            } catch (NoSuchFieldException absent) {
                // Only the actual class hierarchy is searched.
            }
        }
        throw new NoSuchFieldException(name);
    }

    private static int mapNumber(UsageMap map, String name) throws Exception {
        Method method = UsageMap.class.getDeclaredMethod(name);
        method.setAccessible(true);
        return ((Number) method.invoke(map)).intValue();
    }

    private static void map(String role, UsageMap map) throws Exception {
        System.out.printf("MAP\t%s\t%d\t%d\t%d\t%d\t%d\t%d%n", role,
            mapNumber(map, "getTablePageNumber"), mapNumber(map, "getRowStart"),
            mapNumber(map, "getRowEnd"), mapNumber(map, "getStartPage"),
            mapNumber(map, "getEndPage"), map.getPageCount());
        UsageMap.PageCursor cursor = map.cursor();
        cursor.beforeFirst();
        int count = 0;
        for (int page = cursor.getNextPage(); page >= 0; page = cursor.getNextPage()) {
            if (++count > MAX_MAP_MEMBERS) {
                throw new AssertionError("Map member ceiling exceeded");
            }
            System.out.printf("MEMBER\t%s\t%d%n", role, page);
        }
    }

    private static void properties(String owner, PropertyMap values) throws Exception {
        for (PropertyMap.Property property : values) {
            Object value = property.getValue();
            System.out.printf("PROPERTY\t%s\t%s\t%d\t%s\t%s\t%s%n", owner,
                hex(property.getName()), Byte.toUnsignedInt(property.getType().getValue()),
                property.isDdl(), value == null ? "null" : value.getClass().getName(),
                value == null ? "NULL" : hex(value.toString()));
        }
    }

    private static List<Object[]> requests(String selected) {
        if (selected.equals("single")) {
            return List.<Object[]>of(new Object[] {7, new byte[] {(byte) 0x80, 0, (byte) 0xff}});
        }
        if (!selected.equals("batch")) {
            throw new IllegalArgumentException("Closed single/batch request required");
        }
        List<Object[]> rows = new ArrayList<>();
        for (int identity = 100; identity < 116; identity++) {
            byte[] bytes = new byte[255];
            java.util.Arrays.fill(bytes, (byte) 0xe7);
            rows.add(new Object[] {identity, bytes});
        }
        return rows;
    }

    private static void declarations(Table table) throws Exception {
        if (table.getColumnCount() != 2 || table.getIndexes().size() != 2
                || table.getColumn("ID").getType() != DataType.LONG
                || table.getColumn("ID").isAutoNumber()
                || table.getColumn("Bytes").getType() != DataType.BINARY
                || table.getColumn("Bytes").getLength() != 255) {
            throw new AssertionError("Closed two-field declarations required");
        }
        for (Index index : table.getIndexes()) {
            if (!Set.of("PrimaryID", "PrimaryAlias").contains(index.getName())
                    || index.getColumns().size() != 1 || index.shouldIgnoreNulls()
                    || !index.getColumns().get(0).isAscending()
                    || !index.getColumns().get(0).getName().equals("ID")
                    || index.isPrimaryKey() != index.getName().equals("PrimaryID")
                    || !index.isUnique() || !index.isRequired()) {
                throw new AssertionError("Exact required primary alias roles required");
            }
            int number = ((com.healthmarketscience.jackcess.impl.IndexImpl) index)
                .getIndexData().getIndexDataNumber();
            if (number != 0) {
                throw new AssertionError("Exactly one aliased physical index required");
            }
        }
    }

    private static void bounded(Path path) throws Exception {
        long length = Files.size(path);
        if (length <= 0 || length > MAX_FILE_BYTES || length % 4096 != 0) {
            throw new AssertionError("Bounded complete page image required");
        }
    }

    private static void create(Path path) throws Exception {
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Fresh original required");
        }
        try (BoundedFileChannel channel = new BoundedFileChannel(FileChannel.open(path,
                StandardOpenOption.CREATE_NEW, StandardOpenOption.READ,
                StandardOpenOption.WRITE), MAX_FILE_BYTES);
             Database database = new DatabaseBuilder(path).setChannel(channel)
                .setFileFormat(Database.FileFormat.V2016).setAutoSync(false).create()) {
            inert(database);
            Table table = new TableBuilder("Items")
                .addColumn(new ColumnBuilder("ID", DataType.LONG).setAutoNumber(false)
                    .putProperty("Required", true))
                .addColumn(new ColumnBuilder("Bytes", DataType.BINARY).setLength(255))
                .addIndex(new IndexBuilder("PrimaryID").addColumns(true, "ID").setPrimaryKey())
                .addIndex(new IndexBuilder("PrimaryAlias").addColumns(true, "ID")
                    .setUnique().setRequired())
                .toTable(database);
            new TableBuilder("Sentinel")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("Marker", DataType.LONG))
                .toTable(database).addRow(91, 12345);
            declarations(table);
            if (table.getRowCount() != 0) {
                throw new AssertionError("The original Items table must be genuinely empty");
            }
            database.flush();
        }
        bounded(path);
        System.out.println("CREATED\tV2016-empty-owned\t0");
    }

    private static void append(Path source, Path output, String selected) throws Exception {
        bounded(source);
        List<Object[]> values = requests(selected);
        if (Files.exists(output)) {
            throw new IllegalArgumentException("Fresh reference copy required");
        }
        Files.copy(source, output);
        try (BoundedFileChannel channel = new BoundedFileChannel(FileChannel.open(output,
                StandardOpenOption.READ, StandardOpenOption.WRITE), MAX_FILE_BYTES);
             Database database = new DatabaseBuilder(output).setChannel(channel)
                .setAutoSync(false).open()) {
            inert(database);
            Table table = database.getTable("Items");
            declarations(table);
            if (table.getRowCount() != 0) {
                throw new AssertionError("Unchanged empty source required");
            }
            table.addRows(values);
            if (table.getRowCount() != values.size()) {
                throw new AssertionError("Complete closed reference batch required");
            }
            database.flush();
        }
        bounded(output);
        System.out.printf("APPENDED\t%s\t%d%n", selected, values.size());
    }

    private static void inspect(Path path) throws Exception {
        bounded(path);
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            inert(database);
            Set<String> users = database.getTableNames();
            if (!users.equals(Set.of("Items", "Sentinel"))) {
                throw new AssertionError("Exactly the frozen two user tables required");
            }
            TreeSet<String> names = new TreeSet<>(users);
            names.addAll(database.getSystemTableNames());
            if (names.size() > MAX_TABLES) {
                throw new AssertionError("Catalog ceiling exceeded");
            }
            boolean global = false;
            for (String name : names) {
                TableImpl table = (TableImpl) (users.contains(name)
                    ? database.getTable(name) : database.getSystemTable(name));
                String owner = "Table:" + table.getTableDefPageNumber();
                System.out.printf("TABLE\t%s\t%d\t%d\t%d\t%d%n", hex(name),
                    table.getTableDefPageNumber(), table.getRowCount(), table.getColumnCount(),
                    table.getMaxColumnCount());
                if (table.getColumnCount() > MAX_COLUMNS) {
                    throw new AssertionError("Column ceiling exceeded");
                }
                if (!global) {
                    map("GlobalFree", (UsageMap) field(table.getPageChannel(), "_globalUsageMap"));
                    global = true;
                }
                map(owner + ":Owned", (UsageMap) field(table, "_ownedPages"));
                map(owner + ":Hint", (UsageMap) field(table, "_freeSpacePages"));
                properties(owner, table.getProperties());
                for (Column column : table.getColumns()) {
                    ColumnImpl nativeColumn = (ColumnImpl) column;
                    System.out.printf("COLUMN\t%s\t%s\t%d\t%d\t%d\t%d\t%s\t%s\t%d%n",
                        hex(name), hex(column.getName()), column.getColumnIndex(), nativeColumn.getColumnNumber(),
                        Byte.toUnsignedInt(column.getType().getValue()), column.getLength(),
                        column.isAutoNumber(), column.isCompressedUnicode(), nativeColumn.getVarLenTableIndex());
                    properties(owner + ":Column:" + nativeColumn.getColumnNumber(), column.getProperties());
                    if (column.getType() == DataType.MEMO || column.getType() == DataType.OLE) {
                        String role = owner + ":Column:" + nativeColumn.getColumnNumber();
                        Object holder = field(column, "_lvalBufferH");
                        if (holder == null) {
                            System.out.printf("MAP_ABSENT\t%s\tNULL%n", role);
                        } else if (holder.getClass().getName().equals(
                                "com.healthmarketscience.jackcess.impl.LongValueColumnImpl$LegacyLongValueBufferHolder")) {
                            System.out.printf("MAP_ABSENT\t%s\tLEGACY%n", role);
                        } else {
                            if (!holder.getClass().getName().equals(
                                    "com.healthmarketscience.jackcess.impl.LongValueColumnImpl$UmapLongValueBufferHolder")) {
                                throw new AssertionError("Unknown long-value holder implementation");
                            }
                            map(role + ":Owned", (UsageMap) field(holder, "_ownedPages"));
                            map(role + ":Hint", (UsageMap) field(holder, "_freeSpacePages"));
                        }
                    }
                }
                Set<Integer> physical = new HashSet<>();
                for (var index : table.getIndexes()) {
                    IndexData data = index.getIndexData();
                    Method root = IndexData.class.getDeclaredMethod("getRootPageNumber");
                    root.setAccessible(true);
                    System.out.printf("INDEX\t%s\t%s\t%d\t%d\t%d\t%d\t%d\t%d\t%s%n",
                        hex(name), hex(index.getName()), data.getIndexDataNumber(), root.invoke(data),
                        data.getEntryCount(), data.getUniqueEntryCount(), data.getUniqueEntryCountOffset(),
                        Byte.toUnsignedInt(data.getIndexFlags()), index.isPrimaryKey());
                    for (Index.Column column : index.getColumns()) {
                        System.out.printf("KEY_COLUMN\t%s\t%s\t%s\t%s%n", hex(name), hex(index.getName()),
                            hex(column.getName()), column.isAscending());
                    }
                    if (physical.add(data.getIndexDataNumber())) {
                        map(owner + ":IndexData:" + data.getIndexDataNumber(), (UsageMap) field(data, "_ownedPages"));
                    }
                    if (name.equals("Items")) {
                        Cursor cursor = CursorBuilder.createCursor(index);
                        Row row;
                        while ((row = cursor.getNextRow()) != null) {
                            RowIdImpl id = (RowIdImpl) row.getId();
                            System.out.printf("ENTRY\t%s\t%d\t%d\t%d%n", hex(index.getName()),
                                row.getInt("ID"), id.getPageNumber(), id.getRowNumber());
                        }
                    }
                }
                if (!users.contains(name)) {
                    continue;
                }
                if (table.getRowCount() > MAX_ROWS) {
                    throw new AssertionError("User row ceiling exceeded");
                }
                int count = 0;
                for (Row row : table) {
                    int identity = row.getInt("ID");
                    RowIdImpl logical = (RowIdImpl) row.getId();
                    var state = table.createRowState();
                    ByteBuffer body = TableImpl.positionAtRowData(state, logical);
                    RowIdImpl target = state.getFinalRowId();
                    byte[] bytes = new byte[body.remaining()];
                    body.get(bytes);
                    System.out.printf("ROW\t%s\t%d\t%d\t%d\t%d\t%d\t%s%n", hex(name), identity,
                        logical.getPageNumber(), logical.getRowNumber(), target.getPageNumber(), target.getRowNumber(),
                        HexFormat.of().formatHex(bytes));
                    for (Column column : table.getColumns()) {
                        Object value = row.get(column.getName());
                        String tag;
                        String encoded;
                        if (value == null) {
                            tag = "NULL";
                            encoded = "NULL";
                        } else if (value instanceof byte[] binary) {
                            tag = "BINARY";
                            encoded = HexFormat.of().formatHex(binary);
                        } else if (value instanceof String text) {
                            tag = "TEXT";
                            encoded = hex(text);
                        } else if (value instanceof Integer integer) {
                            tag = "INT32";
                            encoded = integer.toString();
                        } else {
                            throw new AssertionError("Unexpected captured user carrier");
                        }
                        System.out.printf("VALUE\t%s\t%d\t%s\t%s\t%s\t%s%n", hex(name), identity,
                            hex(column.getName()), value == null ? "null" : value.getClass().getName(), tag, encoded);
                    }
                    count++;
                }
                if (count != table.getRowCount()) {
                    throw new AssertionError("Complete user getter required");
                }
            }
        }
    }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length == 2 && arguments[0].equals("create")) {
            create(Path.of(arguments[1]));
        } else if (arguments.length == 4 && arguments[0].equals("append")) {
            append(Path.of(arguments[1]), Path.of(arguments[2]), arguments[3]);
        } else if (arguments.length == 2 && arguments[0].equals("inspect")) {
            inspect(Path.of(arguments[1]));
        } else {
            throw new IllegalArgumentException("Closed create/append/inspect arguments required");
        }
    }
}
