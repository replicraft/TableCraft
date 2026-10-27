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
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.HexFormat;
import java.util.List;
import java.util.Set;

/** Original inert producer and explicit public Memo update/read-only oracle. */
public final class CompressedMemoUpdateProbe {
    private static final String TABLE = "CompressedMemos";
    private static final String ID = "ID";
    private static final int SELECTED_ID = 91;
    private static final int SENTINEL_ID = 92;
    private static final int MAX_PAYLOAD_BYTES = 64 * 1024;
    private static final int MAX_ORACLE_BYTES = 1024 * 1024;
    private static final int DESCRIPTOR_BYTES = 12;
    private static final int INLINE_TAG = 0x80000000;
    private static final List<String> MEMOS = List.of("CompactMemo", "RawMemo");
    private static final List<String> LONG_COLUMNS = List.of("CompactMemo", "RawMemo", "OLE");

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setLinkResolver((source, token) -> {
            throw new AssertionError("No linked target is authorized");
        });
    }

    private static String hash(byte[] bytes) throws Exception {
        return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
    }

    private static Object field(Object owner, String name) throws Exception {
        for (Class<?> type = owner.getClass(); type != null; type = type.getSuperclass()) {
            try {
                Field member = type.getDeclaredField(name);
                member.setAccessible(true);
                return member.get(owner);
            } catch (NoSuchFieldException missing) {
                // Search only the actual owning class hierarchy, without fallback values.
            }
        }
        throw new NoSuchFieldException(name);
    }

    private static int mapNumber(UsageMap owner, String name) throws Exception {
        Method getter = UsageMap.class.getDeclaredMethod(name);
        getter.setAccessible(true);
        return ((Number) getter.invoke(owner)).intValue();
    }

    private static void map(String name, UsageMap map) throws Exception {
        if (map == null) {
            throw new IllegalArgumentException("Missing declared usage map");
        }
        System.out.printf("MAP\t%s\t%d\t%d\t%d\t%d%n", name,
            mapNumber(map, "getTablePageNumber"), mapNumber(map, "getRowStart"),
            mapNumber(map, "getRowEnd"), map.getPageCount());
        UsageMap.PageCursor cursor = map.cursor();
        cursor.beforeFirst();
        for (int page = cursor.getNextPage(); page >= 0; page = cursor.getNextPage()) {
            System.out.printf("MEMBER\t%s\t%d%n", name, page);
        }
    }

    private static int root(IndexData index) throws Exception {
        Method getter = IndexData.class.getDeclaredMethod("getRootPageNumber");
        getter.setAccessible(true);
        return ((Number) getter.invoke(index)).intValue();
    }

    private static String value(Object value) {
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

    private static void properties(String owner, PropertyMap properties) {
        for (PropertyMap.Property property : properties) {
            System.out.printf("PROPERTY\t%s\t%s\t%s\t%d\t%s\t%s%n", TABLE, owner,
                property.getName(), Byte.toUnsignedInt(property.getType().getValue()),
                property.isDdl(), value(property.getValue()));
        }
    }

    private static void create(Path path, String profile) throws Exception {
        if (Files.exists(path) || !Set.of("V2016", "V2019").contains(profile)) {
            throw new IllegalArgumentException("Fresh approved ACE16 profile required");
        }
        try (Database database = new DatabaseBuilder(path)
                .setFileFormat(Database.FileFormat.valueOf(profile)).setAutoSync(false).create()) {
            inert(database);
            TableImpl table = (TableImpl) new TableBuilder(TABLE)
                .addColumn(new ColumnBuilder(ID, DataType.LONG))
                .addColumn(new ColumnBuilder("CompactMemo", DataType.MEMO).setCompressedUnicode(true))
                .addColumn(new ColumnBuilder("RawMemo", DataType.MEMO).setCompressedUnicode(false))
                .addColumn(new ColumnBuilder("OLE", DataType.OLE))
                .addIndex(new IndexBuilder("PrimaryID").addColumns(ID).setPrimaryKey())
                .addIndex(new IndexBuilder("IDAsc").addColumns(ID))
                .addIndex(new IndexBuilder("IDTwin").addColumns(ID))
                .toTable(database);
            byte[] sentinel = new byte[256];
            for (int index = 0; index < sentinel.length; index++) {
                sentinel[index] = (byte) index;
            }
            String original = "seed".repeat(1200);
            table.addRow(SELECTED_ID, original, original, sentinel);
            table.addRow(SENTINEL_ID, "untouched-Ā中", "untouched-Ā中", new byte[] {3, 1, 4, 1, 5});
            database.flush();
        }
    }

    private static Cursor selected(TableImpl table) throws Exception {
        Cursor cursor = CursorBuilder.createCursor(table);
        while (cursor.moveToNextRow()) {
            Row row = cursor.getCurrentRow(List.of(ID));
            if (row.getInt(ID) == SELECTED_ID) {
                return cursor;
            }
        }
        throw new IllegalArgumentException("Predeclared ID is absent");
    }

    private static void getter(TableImpl table) throws Exception {
        System.out.printf("TABLE\t%s\t%d\t%d\t%d\t%d%n", TABLE, table.getRowCount(),
            table.getTableDefPageNumber(), table.getColumnCount(), table.getMaxColumnCount());
        properties("$table", table.getProperties());
        for (var column : table.getColumns()) {
            ColumnImpl nativeColumn = (ColumnImpl) column;
            var sort = nativeColumn.getTextSortOrder();
            int lcid = sort == null ? 0 : Short.toUnsignedInt(sort.getValue());
            int variant = sort == null ? 0 : Short.toUnsignedInt(sort.getVariant());
            int version = sort == null ? 0 : Short.toUnsignedInt(sort.getVersion());
            System.out.printf("COLUMN\t%s\t%s\t%d\t%d\t%d\t%d\t%s\t%d\t%d\t%d\t%d%n",
                TABLE, column.getName(), column.getColumnIndex(), nativeColumn.getColumnNumber(),
                Byte.toUnsignedInt(column.getType().getValue()), column.getLength(),
                column.isCompressedUnicode(), nativeColumn.getVarLenTableIndex(), lcid, variant, version);
            properties(column.getName(), column.getProperties());
        }
        for (Row row : table) {
            RowIdImpl id = (RowIdImpl) row.getId();
            List<String> fields = new ArrayList<>();
            fields.add("ROW");
            fields.add(TABLE);
            fields.add(row.getInt(ID).toString());
            fields.add(Integer.toString(id.getPageNumber()));
            fields.add(Integer.toString(id.getRowNumber()));
            for (Column column : table.getColumns()) {
                fields.add(value(row.get(column.getName())));
            }
            System.out.println(String.join("\t", fields));
            ByteBuffer buffer = TableImpl.positionAtRowData(table.createRowState(), id);
            byte[] bytes = new byte[buffer.remaining()];
            buffer.get(bytes);
            System.out.printf("RAWROW\t%s\t%d\t%s%n", TABLE, row.getInt(ID), HexFormat.of().formatHex(bytes));
        }
        for (var index : table.getIndexes()) {
            IndexData data = index.getIndexData();
            System.out.printf("INDEX\t%s\t%s\t%d\t%d\t%d\t%d\t%s\t%s\t%s\t%s%n",
                TABLE, index.getName(), data.getIndexDataNumber(), root(data), data.getEntryCount(),
                data.getOwnedPageCount(), index.isPrimaryKey(), index.isUnique(),
                index.isRequired(), index.shouldIgnoreNulls());
            for (var column : index.getColumns()) {
                System.out.printf("INDEX_COLUMN\t%s\t%s\t%s\t%s%n", TABLE,
                    index.getName(), column.getName(), column.isAscending());
            }
            Cursor cursor = CursorBuilder.createCursor(index);
            Row row;
            while ((row = cursor.getNextRow()) != null) {
                System.out.printf("ENTRY\t%s\t%s\t%d%n", TABLE, index.getName(), row.getInt(ID));
            }
        }
    }

    private static void metadata(TableImpl table) throws Exception {
        var format = table.getFormat();
        System.out.printf("FORMAT\t%d\t%d\t%d\t%d%n", format.PAGE_SIZE,
            format.MAX_INLINE_LONG_VALUE_SIZE, format.MAX_LONG_VALUE_ROW_SIZE, format.SIZE_LONG_VALUE_DEF);
        map("GlobalFree", (UsageMap) field(table.getPageChannel(), "_globalUsageMap"));
        map("TableOwned", (UsageMap) field(table, "_ownedPages"));
        map("TableHint", (UsageMap) field(table, "_freeSpacePages"));
        for (String name : LONG_COLUMNS) {
            Object holder = field(table.getColumn(name), "_lvalBufferH");
            map(name + "Owned", (UsageMap) field(holder, "_ownedPages"));
            map(name + "Hint", (UsageMap) field(holder, "_freeSpacePages"));
        }
        Row row = selected(table).getCurrentRow();
        for (String name : LONG_COLUMNS) {
            Object stored = row.get(name);
            byte[] bytes = stored == null ? new byte[0]
                : stored instanceof String text ? text.getBytes(StandardCharsets.UTF_8) : (byte[]) stored;
            System.out.printf("VALUE\t%s\t%s\t%d\t%s%n", name,
                stored == null ? "NULL" : "PRESENT", bytes.length, hash(bytes));
        }
    }

    private static Object replacement(String kind, Path payload) throws Exception {
        if (Files.size(payload) > MAX_PAYLOAD_BYTES) {
            throw new IllegalArgumentException("Payload exceeds frozen bound");
        }
        byte[] bytes = Files.readAllBytes(payload);
        if (bytes.length > MAX_PAYLOAD_BYTES) {
            throw new IllegalArgumentException("Payload changed beyond frozen bound");
        }
        if (kind.equals("null")) {
            if (bytes.length != 0) {
                throw new IllegalArgumentException("NULL payload is not empty");
            }
            return null;
        }
        if (!kind.equals("text")) {
            throw new IllegalArgumentException("Only NULL/Text Memo replacements are authorized");
        }
        String text = new String(bytes, StandardCharsets.UTF_8);
        if (!Arrays.equals(bytes, text.getBytes(StandardCharsets.UTF_8))) {
            throw new IllegalArgumentException("Payload is not faithful UTF8");
        }
        return text;
    }

    private static void decode(TableImpl table, Path input) throws Exception {
        if (Files.size(input) > MAX_ORACLE_BYTES) {
            throw new IllegalArgumentException("Oracle input exceeds frozen bound");
        }
        for (String line : Files.readAllLines(input, StandardCharsets.US_ASCII)) {
            String[] fields = line.split("\t", -1);
            if (fields.length != 3 || !MEMOS.contains(fields[1])) {
                throw new IllegalArgumentException("Invalid bounded decoder record");
            }
            byte[] stream = HexFormat.of().parseHex(fields[2]);
            if (stream.length > MAX_PAYLOAD_BYTES) {
                throw new IllegalArgumentException("Encoded oracle stream too large");
            }
            ByteBuffer descriptor = ByteBuffer.allocate(DESCRIPTOR_BYTES + stream.length)
                .order(ByteOrder.LITTLE_ENDIAN).putInt(INLINE_TAG | stream.length).putLong(0).put(stream);
            String actual = (String) ((ColumnImpl) table.getColumn(fields[1])).read(descriptor.array());
            System.out.printf("DECODE\t%s\t%s\t%s%n", fields[0], fields[1], value(actual));
        }
    }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length < 2) {
            throw new IllegalArgumentException("Explicit mode and path are required");
        }
        String mode = arguments[0];
        Path path = Path.of(arguments[1]);
        if (mode.equals("create")) {
            if (arguments.length != 3) {
                throw new IllegalArgumentException("create PATH PROFILE");
            }
            create(path, arguments[2]);
            return;
        }
        boolean writing = mode.equals("update");
        if (writing && (arguments.length != 5 || !MEMOS.contains(arguments[2]))) {
            throw new IllegalArgumentException("update PATH COLUMN KIND PAYLOAD");
        }
        if (!writing && !(Set.of("getter", "metadata", "inspect").contains(mode) && arguments.length == 2)
                && !(mode.equals("decode") && arguments.length == 3)) {
            throw new IllegalArgumentException("Unrecognized explicit operation");
        }
        Object replacement = writing ? replacement(arguments[3], Path.of(arguments[4])) : null;
        try (Database database = new DatabaseBuilder(path).setReadOnly(!writing).open()) {
            inert(database);
            TableImpl table = (TableImpl) database.getTable(TABLE);
            if (writing) {
                var values = new HashMap<String, Object>();
                values.put(arguments[2], replacement);
                selected(table).updateCurrentRowFromMap(values);
            } else if (mode.equals("decode")) {
                decode(table, Path.of(arguments[2]));
            } else {
                if (!mode.equals("metadata")) {
                    getter(table);
                }
                if (!mode.equals("getter")) {
                    metadata(table);
                }
            }
        }
    }
}
