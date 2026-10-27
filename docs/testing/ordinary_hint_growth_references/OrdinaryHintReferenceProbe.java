import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.Cursor;
import com.healthmarketscience.jackcess.CursorBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Index;
import com.healthmarketscience.jackcess.PropertyMap;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import com.healthmarketscience.jackcess.impl.IndexData;
import com.healthmarketscience.jackcess.impl.IndexImpl;
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
import java.security.MessageDigest;
import java.util.Arrays;
import java.util.HexFormat;
import java.util.HashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.TimeZone;

/** Repository-authored bounded MAX reference and complete inert five-table getter. */
public final class OrdinaryHintReferenceProbe {
    private static final List<String> SELECTED = List.of("TextLegacy", "TextModern", "BinaryOne");
    private static final int ROWS = 900;
    private static final int DECLARED_LENGTH = 255;
    private static final int MARKER_FACTOR = 17;
    private static final int MAX_MAP_MEMBERS = 32736;
    private static final long MAX_FILE_BYTES = 8L * 1024 * 1024;
    private static final String TOKEN = "{00112233-4455-6677-8899-AABBCCDDEEFF}";
    private static final String SENTINEL = "LongSentinel";
    private static final String PADDING = "PaddingHint";
    private static final int PADDING_COLUMNS = 7;
    private static final int PADDING_UNITS = 255;
    private static final int MAX_SOURCE_ROWS = 4096;

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setEnforceForeignKeys(true);
        database.setReuseFreePages(false);
        database.setTimeZone(TimeZone.getTimeZone("UTC"));
        database.setLinkResolver((source, token) -> {
            throw new AssertionError("Linked targets are not authorized");
        });
    }

    private static void bounded(Path path) throws Exception {
        if (Files.size(path) > MAX_FILE_BYTES) {
            throw new IllegalArgumentException("Frozen file limit exceeded");
        }
    }

    private static Object original(String table) {
        return table.equals("BinaryOne") ? new byte[] {0} : "aaa";
    }

    private static Object replacement(String table) {
        return table.equals("BinaryOne") ? filledBinary() : "z".repeat(DECLARED_LENGTH);
    }

    private static byte[] filledBinary() {
        byte[] bytes = new byte[DECLARED_LENGTH];
        Arrays.fill(bytes, (byte) 0xff);
        return bytes;
    }

    private static int padCount(String table) {
        return table.equals("BinaryOne") ? 8 : 4;
    }

    private static boolean same(Object left, Object right) {
        if (left instanceof byte[] a && right instanceof byte[] b) {
            return Arrays.equals(a, b);
        }
        return left.equals(right);
    }

    private static String value(Object value, DataType type) {
        if (type == DataType.GUID) {
            return "Guid:" + value.toString().toUpperCase(Locale.ROOT);
        }
        if (type == DataType.BYTE) {
            return "Byte:" + (((Number) value).intValue() & 0xff);
        }
        if (value instanceof byte[] bytes) {
            return "Binary:" + HexFormat.of().formatHex(bytes);
        }
        if (value instanceof String text) {
            return "Text:" + HexFormat.of().formatHex(text.getBytes(StandardCharsets.UTF_16LE));
        }
        return "Int32:" + value;
    }

    private static String hash(Path path) throws Exception {
        byte[] bytes = Files.readAllBytes(path);
        return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
    }

    private static void reference(Path source, Path output, String selected, int paddingRows) throws Exception {
        bounded(source);
        if (!Set.of(384, 700).contains(paddingRows)) {
            throw new IllegalArgumentException("Frozen padding selector required");
        }
        String originalHash = hash(source);
        if (!SELECTED.contains(selected) || Files.exists(output)
                || source.toRealPath().equals(output.toAbsolutePath().normalize())) {
            throw new IllegalArgumentException("Fresh distinct reference and frozen selector required");
        }
        Files.copy(source, output);
        try (BoundedFileChannel channel = new BoundedFileChannel(FileChannel.open(output,
                StandardOpenOption.READ, StandardOpenOption.WRITE), MAX_FILE_BYTES);
             Database database = new DatabaseBuilder(output).setChannel(channel).setAutoSync(false).open()) {
            inert(database);
            if (!database.getTableNames().equals(Set.of("TextLegacy", "TextModern", "BinaryOne", SENTINEL, PADDING))
                    || database.getTable(PADDING).getRowCount() != paddingRows) {
                throw new IllegalArgumentException("Qualified five-table source differs");
            }
            Table table = database.getTable(selected);
            if (table.getRowCount() != ROWS) {
                throw new IllegalArgumentException("Frozen selected population changed");
            }
            Cursor cursor = CursorBuilder.createCursor(table);
            boolean changed = false;
            while (cursor.moveToNextRow()) {
                Row row = cursor.getCurrentRow();
                if (row.getInt("ID") != 1) {
                    continue;
                }
                if (changed || !same(row.get("KeyValue"), original(selected))
                        || row.getInt("Marker") != MARKER_FACTOR) {
                    throw new IllegalArgumentException("Selected expected value changed");
                }
                Map<String, Object> update = new HashMap<>();
                update.put("KeyValue", replacement(selected));
                cursor.updateCurrentRowFromMap(update);
                changed = true;
            }
            if (!changed) {
                throw new IllegalArgumentException("Selected identity absent");
            }
            database.flush();
        }
        bounded(output);
        if (!hash(source).equals(originalHash)) {
            throw new AssertionError("Immutable enlarged source changed");
        }
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

    private static void properties(String owner, PropertyMap properties) throws Exception {
        for (PropertyMap.Property property : properties) {
            System.out.printf("PROPERTY\t%s\t%s\t%d\t%s\t%s%n", owner, property.getName(),
                Byte.toUnsignedInt(property.getType().getValue()), property.isDdl(),
                HexFormat.of().formatHex(String.valueOf(property.getValue()).getBytes(StandardCharsets.UTF_16LE)));
        }
    }

    private static void map(String role, UsageMap map) throws Exception {
        Method container = UsageMap.class.getDeclaredMethod("getTablePageNumber");
        Method start = UsageMap.class.getDeclaredMethod("getRowStart");
        Method end = UsageMap.class.getDeclaredMethod("getRowEnd");
        container.setAccessible(true);
        start.setAccessible(true);
        end.setAccessible(true);
        System.out.printf("MAP\t%s\t%d\t%d\t%d\t%d%n", role, container.invoke(map),
            start.invoke(map), end.invoke(map), map.getPageCount());
        UsageMap.PageCursor cursor = map.cursor();
        cursor.beforeFirst();
        int count = 0;
        for (int page = cursor.getNextPage(); page >= 0; page = cursor.getNextPage()) {
            if (++count > MAX_MAP_MEMBERS) {
                throw new IllegalArgumentException("Map capture bound exceeded");
            }
            System.out.printf("MEMBER\t%s\t%d%n", role, page);
        }
    }

    private static void index(String table, Index index) throws Exception {
        IndexData data = ((IndexImpl) index).getIndexData();
        Method root = IndexData.class.getDeclaredMethod("getRootPageNumber");
        root.setAccessible(true);
        System.out.printf("INDEX\t%s\t%s\t%d\t%d\t%d\t%d\t%s\t%s\t%s%n", table, index.getName(),
            data.getIndexDataNumber(), root.invoke(data), data.getEntryCount(),
            Byte.toUnsignedInt(data.getIndexFlags()), index.isPrimaryKey(), index.isUnique(), index.isForeignKey());
        for (Index.Column column : index.getColumns()) {
            System.out.printf("INDEX_COLUMN\t%s\t%s\t%d\t%s%n", table, index.getName(),
                ((ColumnImpl) column.getColumn()).getColumnNumber(), column.isAscending());
        }
        data.validate(true);
        map(table + ":Index:" + index.getName(), (UsageMap) field(data, "_ownedPages"));

        Method entryBytes = IndexData.Entry.class.getDeclaredMethod("getEntryBytes");
        entryBytes.setAccessible(true);
        IndexData.EntryCursor cursor = data.cursor();
        IndexData.Entry sentinel = cursor.getLastEntry();
        int count = 0;
        for (IndexData.Entry entry = cursor.getNextEntry(); !entry.equals(sentinel); entry = cursor.getNextEntry()) {
            if (++count > ROWS) {
                throw new IllegalArgumentException("Entry capture bound exceeded");
            }
            RowIdImpl row = entry.getRowId();
            System.out.printf("KEY\t%s\t%s\t%d\t%d\t%s%n", table, index.getName(),
                row.getPageNumber(), row.getRowNumber(),
                HexFormat.of().formatHex((byte[]) entryBytes.invoke(entry)));
        }
        if (count != (table.equals(SENTINEL) ? 1 : ROWS)) {
            throw new AssertionError("Complete index entry count differs");
        }
        if (!table.equals(SENTINEL) && !index.getName().equals("PrimaryID")) {
            Method encode = IndexData.class.getDeclaredMethod("createEntryBytes", Object[].class);
            encode.setAccessible(true);
            Object[] beforeRow = data.constructIndexRow("KeyValue", original(table));
            Object[] afterRow = data.constructIndexRow("KeyValue", replacement(table));
            byte[] before = (byte[]) encode.invoke(data, new Object[] {beforeRow});
            byte[] after = (byte[]) encode.invoke(data, new Object[] {afterRow});
            System.out.printf("ENCODE\t%s\t%s\t%s\t%s%n", table, index.getName(),
                HexFormat.of().formatHex(before), HexFormat.of().formatHex(after));
        }
    }

    private static void inspect(Path path, int paddingRows, String changedTable) throws Exception {
        bounded(path);
        if (!Set.of(384, 700).contains(paddingRows)) {
            throw new IllegalArgumentException("Frozen padding selector required");
        }
        if (!SELECTED.contains(changedTable)) {
            throw new IllegalArgumentException("Frozen changed-table selector required");
        }
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            inert(database);
            if (!database.getTableNames().equals(Set.of("TextLegacy", "TextModern", "BinaryOne", SENTINEL, PADDING))) {
                throw new AssertionError("Frozen user-table namespace changed");
            }
            TableImpl first = (TableImpl) database.getTable("TextLegacy");
            map("GlobalFree", (UsageMap) field(first.getPageChannel(), "_globalUsageMap"));

            for (String name : List.of("TextLegacy", "TextModern", "BinaryOne", SENTINEL, PADDING)) {
                TableImpl table = (TableImpl) database.getTable(name);
                System.out.printf("TABLE\t%s\t%d\t%d%n", name, table.getTableDefPageNumber(), table.getRowCount());
                map(name + ":Owned", (UsageMap) field(table, "_ownedPages"));
                map(name + ":Hint", (UsageMap) field(table, "_freeSpacePages"));
                properties(name, table.getProperties());
                for (Column column : table.getColumns()) {
                    ColumnImpl nativeColumn = (ColumnImpl) column;
                    ColumnImpl.SortOrder order = nativeColumn.getTextSortOrder();
                    String descriptor = order == null ? "none"
                        : order.getValue() + ":" + order.getVariant() + ":" + order.getVersion();
                    System.out.printf("COLUMN\t%s\t%s\t%d\t%d\t%d\t%s\t%s\t%d\t%d%n", name,
                        column.getName(), nativeColumn.getColumnNumber(),
                        Byte.toUnsignedInt(column.getType().getValue()), column.getLength(),
                        column.isCompressedUnicode(), descriptor,
                        column.getColumnIndex(), nativeColumn.getVarLenTableIndex());
                    properties(name + ":" + column.getName(), column.getProperties());
                    if (column.getType() == DataType.MEMO || column.getType() == DataType.OLE) {
                        Object holder = field(column, "_lvalBufferH");
                        if (holder == null) {
                            System.out.printf("MAP_STATUS\t%s:%s\tAbsentHolder%n", name, column.getName());
                        } else {
                            map(name + ":" + column.getName() + ":Owned", (UsageMap) field(holder, "_ownedPages"));
                            map(name + ":" + column.getName() + ":Hint", (UsageMap) field(holder, "_freeSpacePages"));
                        }
                    }
                }
                int count = 0;
                for (Row row : table) {
                    int identity = row.getInt("ID");
                    if (identity != ++count) {
                        throw new AssertionError("Native row traversal order changed");
                    }
                    if (name.equals(PADDING)) {
                        if (identity > paddingRows) {
                            throw new AssertionError("Padding row bound exceeded");
                        }
                        for (int column = 1; column <= PADDING_COLUMNS; column++) {
                            if (!"Ā".repeat(PADDING_UNITS).equals(row.getString("Wide" + column))) {
                                throw new AssertionError("Full padding value differs");
                            }
                        }
                    } else if (!name.equals(SENTINEL)) {
                        Object expected = name.equals(changedTable) && identity == 1
                            ? replacement(name) : original(name);
                        if (!same(row.get("KeyValue"), expected) || row.getInt("Marker") != identity * MARKER_FACTOR) {
                            throw new AssertionError("Actual typed source/reference value differs");
                        }
                        if (!TOKEN.equalsIgnoreCase(row.getString("Token"))) {
                            throw new AssertionError("GUID companion changed");
                        }
                        for (int pad = 1; pad <= padCount(name); pad++) {
                            int actual = ((Number) row.get("Pad" + pad)).intValue() & 0xff;
                            if (actual != ((identity + MARKER_FACTOR * pad) & 0xff)) {
                                throw new AssertionError("BYTE companion changed");
                            }
                        }
                    } else if (identity != 1 || !"Ā".repeat(33).equals(row.getString("Memo"))
                            || !Arrays.equals(new byte[] {0, (byte) 0xff, 0x41}, (byte[]) row.get("Ole"))) {
                        throw new AssertionError("Long-value sentinel changed");
                    }
                    RowIdImpl id = (RowIdImpl) row.getId();
                    var state = table.createRowState();
                    ByteBuffer record = TableImpl.positionAtRowData(state, id);
                    RowIdImpl target = state.getFinalRowId();
                    System.out.printf("LOCATION\t%s\t%d\t%d\t%d\t%d\t%d%n", name, identity,
                        id.getPageNumber(), id.getRowNumber(), target.getPageNumber(), target.getRowNumber());
                    byte[] raw = new byte[record.remaining()];
                    record.get(raw);
                    System.out.printf("ROW\t%s\t%d\t%d\t%d\t%s%n", name, identity,
                        id.getPageNumber(), id.getRowNumber(), HexFormat.of().formatHex(raw));
                    for (Column column : table.getColumns()) {
                        System.out.printf("VALUE\t%s\t%d\t%s\t%s%n", name, identity,
                            column.getName(), value(row.get(column.getName()), column.getType()));
                    }
                }
                if (count != (name.equals(PADDING) ? paddingRows : (name.equals(SENTINEL) ? 1 : ROWS))) {
                    throw new AssertionError("Frozen row count changed");
                }
                for (Index index : table.getIndexes()) {
                    index(name, index);
                }
            }
            System.out.printf("POLICY\t%s\t%s\t%s\t%s%n", database.isEvaluateExpressions(),
                database.isEnforceForeignKeys(), database.isReuseFreePages(), database.getTimeZone().getID());
            if (database.isEvaluateExpressions() || !database.isEnforceForeignKeys()
                    || database.isReuseFreePages() || !database.getTimeZone().getID().equals("UTC")) {
                throw new AssertionError("Inert policies changed");
            }
        }
    }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length == 5 && arguments[0].equals("reference")) {
            reference(Path.of(arguments[1]), Path.of(arguments[2]), arguments[3], Integer.parseInt(arguments[4]));
        } else if (arguments.length == 4 && arguments[0].equals("inspect")) {
            inspect(Path.of(arguments[1]), Integer.parseInt(arguments[2]), arguments[3]);
        } else {
            throw new IllegalArgumentException("Operation outside frozen MAX-reference recipe");
        }
    }
}
