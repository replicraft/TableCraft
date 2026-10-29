import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.ConstraintViolationException;
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

/** Fixed public IgnoreNULL originals and explicitly ordered reference updates. */
public final class IgnoredNullUpdateFixture {
    private static final long MAX_FILE_BYTES = 8L * 1024 * 1024;
    private static final int MAX_MAP_MEMBERS = 32736;
    private static final int MAX_TABLES = 64;
    private static final int MAX_COLUMNS = 256;
    private static final int MAX_ROWS = 4096;

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setAllowAutoNumberInsert(false);
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

    private static void bounded(Path path) throws Exception {
        long length = Files.size(path);
        if (length <= 0 || length > MAX_FILE_BYTES || length % 4096 != 0) {
            throw new AssertionError("Bounded complete page image required");
        }
    }

    private static Database.FileFormat profile(String selected) {
        if (!Set.of("V2016", "V2019").contains(selected)) {
            throw new IllegalArgumentException("Exactly the two ACE 16 layouts required");
        }
        return Database.FileFormat.valueOf(selected);
    }

    private static int last(TableImpl table, String name) throws Exception {
        Method getter = TableImpl.class.getDeclaredMethod(name);
        getter.setAccessible(true);
        return ((Number) getter.invoke(table)).intValue();
    }

    private static IndexBuilder secondary(String name, boolean tuple, boolean unique,
            boolean required) {
        IndexBuilder index = new IndexBuilder(name).setIgnoreNulls();
        if (tuple) {
            index.addColumns(false, "Right").addColumns(true, "Left");
        } else {
            index.addColumns(true, "Left");
        }
        if (unique) {
            index.setUnique();
        }
        if (required) {
            index.setRequired();
        }
        return index;
    }

    private static void table(Database database, String name, boolean tuple, boolean unique,
            boolean required, Object[][] rows) throws Exception {
        Table table = new TableBuilder(name)
            .addColumn(new ColumnBuilder("ID", DataType.LONG))
            .addColumn(new ColumnBuilder("Left", DataType.LONG))
            .addColumn(new ColumnBuilder("Right", DataType.LONG))
            .addColumn(new ColumnBuilder("Marker", DataType.LONG))
            .addIndex(new IndexBuilder("PrimaryID").addColumns("ID").setPrimaryKey())
            .addIndex(new IndexBuilder("PrimaryAlias").addColumns("ID").setRequired().setUnique())
            .addIndex(secondary("ValueOrder", tuple, unique, required))
            .addIndex(secondary("ValueAlias", tuple, unique, required))
            .toTable(database);
        for (Object[] row : rows) {
            table.addRow(row);
        }
    }

    private static void create(Path path, String layout) throws Exception {
        Database.FileFormat format = profile(layout);
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Fresh original required");
        }
        try (BoundedFileChannel channel = new BoundedFileChannel(FileChannel.open(path,
                StandardOpenOption.CREATE_NEW, StandardOpenOption.READ,
                StandardOpenOption.WRITE), MAX_FILE_BYTES);
             Database database = new DatabaseBuilder(path).setChannel(channel)
                .setFileFormat(format).setAutoSync(false).create()) {
            inert(database);
            table(database, "Single", false, false, false,
                new Object[][] {
                    {1, 10, 100, 1001},
                    {2, 10, 200, 1002},
                    {3, null, 300, 1003},
                    {4, 20, null, 1004},
                });
            table(database, "Tuple", true, false, false,
                new Object[][] {
                    {1, 10, 100, 1001},
                    {2, 10, 100, 1002},
                    {3, null, null, 1003},
                    {4, 20, 200, 1004},
                    {5, 10, null, 1005},
                    {6, null, 100, 1006},
                });
            table(database, "UniqueSingle", false, true, false,
                new Object[][] {
                    {1, 10, 100, 1001},
                    {2, 20, 200, 1002},
                    {3, null, 300, 1003},
                    {4, null, null, 1004},
                });
            table(database, "UniqueTuple", true, true, false,
                new Object[][] {
                    {1, 10, 100, 1001},
                    {2, 20, 200, 1002},
                    {3, null, null, 1003},
                    {4, 10, null, 1004},
                    {5, null, 100, 1005},
                });
            table(database, "RequiredSingle", false, false, true,
                new Object[][] {
                    {1, 10, 100, 1001},
                    {2, 20, null, 1002},
                });
            table(database, "RequiredTuple", true, false, true,
                new Object[][] {
                    {1, 10, 100, 1001},
                    {2, 20, 200, 1002},
                });
            new TableBuilder("Sentinel")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("Marker", DataType.LONG))
                .toTable(database).addRow(91, -12345);
            database.flush();
        }
        bounded(path);
        System.out.printf("CREATED\t%s%n", layout);
    }

    private static void update(Table table, int identity, String[] columns, Integer[] values)
            throws Exception {
        Row selected = null;
        for (Row row : table) {
            if (row.getInt("ID") == identity) {
                if (selected != null) {
                    throw new AssertionError("Unique original identity required");
                }
                selected = row;
            }
        }
        if (selected == null || columns.length != values.length) {
            throw new AssertionError("Closed original update target required");
        }
        for (int position = 0; position < columns.length; position++) {
            selected.put(columns[position], values[position]);
        }
        table.updateRow(selected);
    }

    private static void action(Path source, Path output, String layout, String selected)
            throws Exception {
        profile(layout);
        bounded(source);
        if (Files.exists(output)) {
            throw new IllegalArgumentException("Fresh reference copy required");
        }
        Set<String> actions = Set.of(
            "single-null-new",
            "single-null-existing",
            "single-present-null",
            "single-removed-family-forward",
            "single-removed-family-reverse",
            "single-cointroduced-forward",
            "single-cointroduced-reverse",
            "tuple-null-new",
            "tuple-null-existing",
            "tuple-null-partial",
            "tuple-partial-omitted",
            "tuple-partial-present",
            "unique-single-duplicate",
            "unique-tuple-partial-duplicate",
            "required-single-all-null",
            "required-tuple-all-null",
            "required-tuple-partial-null");
        if (!actions.contains(selected)) {
            throw new IllegalArgumentException("Closed reference action required");
        }
        Files.copy(source, output);
        try (BoundedFileChannel channel = new BoundedFileChannel(FileChannel.open(output,
                StandardOpenOption.READ, StandardOpenOption.WRITE), MAX_FILE_BYTES);
             Database database = new DatabaseBuilder(output).setChannel(channel)
                .setAutoSync(false).open()) {
            inert(database);
            boolean refused = false;
            try {
                switch (selected) {
                    case "single-null-new" -> {
                        update(database.getTable("Single"), 3,
                            new String[] {"Left"}, new Integer[] {30});
                    }
                    case "single-null-existing" -> {
                        update(database.getTable("Single"), 3,
                            new String[] {"Left"}, new Integer[] {10});
                    }
                    case "single-present-null" -> {
                        update(database.getTable("Single"), 1,
                            new String[] {"Left"}, new Integer[] {null});
                    }
                    case "single-removed-family-forward" -> {
                        update(database.getTable("Single"), 1,
                            new String[] {"Left"}, new Integer[] {null});
                        update(database.getTable("Single"), 2,
                            new String[] {"Left"}, new Integer[] {null});
                        update(database.getTable("Single"), 3,
                            new String[] {"Left"}, new Integer[] {10});
                    }
                    case "single-removed-family-reverse" -> {
                        update(database.getTable("Single"), 3,
                            new String[] {"Left"}, new Integer[] {10});
                        update(database.getTable("Single"), 1,
                            new String[] {"Left"}, new Integer[] {null});
                        update(database.getTable("Single"), 2,
                            new String[] {"Left"}, new Integer[] {null});
                    }
                    case "single-cointroduced-forward" -> {
                        update(database.getTable("Single"), 1,
                            new String[] {"Left"}, new Integer[] {30});
                        update(database.getTable("Single"), 3,
                            new String[] {"Left"}, new Integer[] {30});
                    }
                    case "single-cointroduced-reverse" -> {
                        update(database.getTable("Single"), 3,
                            new String[] {"Left"}, new Integer[] {30});
                        update(database.getTable("Single"), 1,
                            new String[] {"Left"}, new Integer[] {30});
                    }
                    case "tuple-null-new" -> {
                        update(database.getTable("Tuple"), 3,
                            new String[] {"Left", "Right"}, new Integer[] {30, 300});
                    }
                    case "tuple-null-existing" -> {
                        update(database.getTable("Tuple"), 3,
                            new String[] {"Left", "Right"}, new Integer[] {10, 100});
                    }
                    case "tuple-null-partial" -> {
                        update(database.getTable("Tuple"), 3,
                            new String[] {"Left", "Right"}, new Integer[] {30, null});
                    }
                    case "tuple-partial-omitted" -> {
                        update(database.getTable("Tuple"), 5,
                            new String[] {"Left", "Right"}, new Integer[] {null, null});
                    }
                    case "tuple-partial-present" -> {
                        update(database.getTable("Tuple"), 6,
                            new String[] {"Left", "Right"}, new Integer[] {20, 100});
                    }
                    case "unique-single-duplicate" -> {
                        update(database.getTable("UniqueSingle"), 3,
                            new String[] {"Left"}, new Integer[] {10});
                    }
                    case "unique-tuple-partial-duplicate" -> {
                        update(database.getTable("UniqueTuple"), 3,
                            new String[] {"Left", "Right"}, new Integer[] {10, null});
                    }
                    case "required-single-all-null" -> {
                        update(database.getTable("RequiredSingle"), 1,
                            new String[] {"Left"}, new Integer[] {null});
                    }
                    case "required-tuple-all-null" -> {
                        update(database.getTable("RequiredTuple"), 1,
                            new String[] {"Left", "Right"}, new Integer[] {null, null});
                    }
                    case "required-tuple-partial-null" -> {
                        update(database.getTable("RequiredTuple"), 1,
                            new String[] {"Left", "Right"}, new Integer[] {null, 100});
                    }
                    default -> throw new AssertionError("Closed action dispatch required");
                }
            } catch (ConstraintViolationException expected) {
                if (!Set.of("unique-single-duplicate", "unique-tuple-partial-duplicate",
                        "required-tuple-partial-null").contains(selected)) {
                    throw expected;
                }
                refused = true;
            }
            boolean expectedRefusal = Set.of("unique-single-duplicate",
                "unique-tuple-partial-duplicate", "required-tuple-partial-null").contains(selected);
            if (refused != expectedRefusal) {
                throw new AssertionError("Contrary public constraint result; preserve first output");
            }
            database.flush();
            System.out.printf("ACTION\t%s\t%s%n", selected,
                refused ? "ConstraintViolationException" : "success");
        }
        bounded(output);
    }

    private static void inspect(Path path, String layout) throws Exception {
        profile(layout);
        bounded(path);
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            inert(database);
            Set<String> users = database.getTableNames();
            if (!users.equals(Set.of("Single", "Tuple", "UniqueSingle", "UniqueTuple", "RequiredSingle", "RequiredTuple", "Sentinel"))) {
                throw new AssertionError("Exactly the frozen seven user tables required");
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
                System.out.printf("AUTONUMBER\t%s\t%d\t%d\t%s%n", hex(name),
                    last(table, "getLastLongAutoNumber"),
                    last(table, "getLastComplexTypeAutoNumber"), table.isAllowAutoNumberInsert());
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
                    if (users.contains(name) && !name.equals("Sentinel")) {
                        Cursor cursor = CursorBuilder.createCursor(index);
                        Row row;
                        while ((row = cursor.getNextRow()) != null) {
                            RowIdImpl id = (RowIdImpl) row.getId();
                            System.out.printf("ENTRY\t%s\t%s\t%d\t%d\t%d%n", hex(name), hex(index.getName()),
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
        if (arguments.length == 3 && arguments[0].equals("create")) {
            create(Path.of(arguments[1]), arguments[2]);
        } else if (arguments.length == 3 && arguments[0].equals("inspect")) {
            inspect(Path.of(arguments[1]), arguments[2]);
        } else if (arguments.length == 5 && arguments[0].equals("action")) {
            action(Path.of(arguments[1]), Path.of(arguments[2]), arguments[3], arguments[4]);
        } else {
            throw new IllegalArgumentException("Closed create/inspect/action arguments required");
        }
    }
}
