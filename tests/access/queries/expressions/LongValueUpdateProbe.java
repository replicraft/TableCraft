import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.Cursor;
import com.healthmarketscience.jackcess.CursorBuilder;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.impl.TableImpl;
import com.healthmarketscience.jackcess.impl.UsageMap;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.HexFormat;
import java.util.HashMap;
import java.util.List;

/** Explicit passive metadata capture or one separately authorized public update. */
public final class LongValueUpdateProbe {
    private static final String TABLE = "Sentinel";
    private static final String ID = "ID";
    private static final int IDENTITY = 91;
    private static final int MAX_PAYLOAD_BYTES = 64 * 1024;

    private static Object field(Object owner, String name) throws Exception {
        for (Class<?> type = owner.getClass(); type != null; type = type.getSuperclass()) {
            try {
                Field member = type.getDeclaredField(name);
                member.setAccessible(true);
                return member.get(owner);
            } catch (NoSuchFieldException missing) {
                // The owning field may belong to a superclass. A missing field
                // after the complete search is an explicit qualification error.
            }
        }
        throw new NoSuchFieldException(name);
    }

    private static int number(Object owner, String name) throws Exception {
        Method getter = UsageMap.class.getDeclaredMethod(name);
        getter.setAccessible(true);
        return ((Number) getter.invoke(owner)).intValue();
    }

    private static String hash(byte[] bytes) throws Exception {
        return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
    }

    private static void map(String name, UsageMap map) throws Exception {
        if (map == null) {
            throw new IllegalArgumentException("Missing qualified usage map");
        }
        System.out.printf("MAP\t%s\t%d\t%d\t%d\t%d%n", name,
            number(map, "getTablePageNumber"), number(map, "getRowStart"),
            number(map, "getRowEnd"), map.getPageCount());
        UsageMap.PageCursor cursor = map.cursor();
        cursor.beforeFirst();
        for (int page = cursor.getNextPage(); page >= 0; page = cursor.getNextPage()) {
            System.out.printf("MEMBER\t%s\t%d%n", name, page);
        }
    }

    private static Cursor selected(TableImpl table) throws Exception {
        Cursor cursor = CursorBuilder.createCursor(table);
        while (cursor.moveToNextRow()) {
            Row row = cursor.getCurrentRow(List.of(ID));
            if (row.getInt(ID) == IDENTITY) {
                return cursor;
            }
        }
        throw new IllegalArgumentException("Predeclared row absent");
    }

    private static Object replacement(String kind, Path payload) throws Exception {
        if (Files.size(payload) > MAX_PAYLOAD_BYTES) {
            throw new IllegalArgumentException("Payload exceeds observation bound");
        }
        byte[] bytes = Files.readAllBytes(payload);
        if (bytes.length > MAX_PAYLOAD_BYTES) {
            throw new IllegalArgumentException("Payload exceeds observation bound");
        }
        return switch (kind) {
            case "null" -> {
                if (bytes.length != 0) {
                    throw new IllegalArgumentException("NULL payload must be empty");
                }
                yield null;
            }
            case "text" -> {
                String value = new String(bytes, StandardCharsets.UTF_8);
                if (!java.util.Arrays.equals(value.getBytes(StandardCharsets.UTF_8), bytes)) {
                    throw new IllegalArgumentException("Text must be faithful UTF-8");
                }
                yield value;
            }
            case "binary" -> bytes;
            default -> throw new IllegalArgumentException("Unknown payload kind");
        };
    }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length != 2 && arguments.length != 5) {
            throw new IllegalArgumentException("metadata PATH or update PATH COLUMN KIND PAYLOAD");
        }
        boolean writing = arguments[0].equals("update");
        if (writing != (arguments.length == 5)
            || (!writing && !arguments[0].equals("metadata"))) {
            throw new IllegalArgumentException("Explicit operation required");
        }
        if (writing && !List.of("Memo", "OLE").contains(arguments[2])) {
            throw new IllegalArgumentException("Column outside frozen plan");
        }
        if (writing && !arguments[3].equals("null")
            && !(arguments[2].equals("Memo") && arguments[3].equals("text"))
            && !(arguments[2].equals("OLE") && arguments[3].equals("binary"))) {
            throw new IllegalArgumentException("Column/carrier mismatch");
        }
        Object value = writing ? replacement(arguments[3], Path.of(arguments[4])) : null;
        Path path = Path.of(arguments[1]);
        try (Database database = new DatabaseBuilder(path).setReadOnly(!writing).open()) {
            database.setEvaluateExpressions(false);
            database.setLinkResolver((source, token) -> {
                throw new AssertionError("No linked target authorized");
            });
            TableImpl table = (TableImpl) database.getTable(TABLE);
            Cursor cursor = selected(table);
            if (writing) {
                var change = new HashMap<String, Object>();
                change.put(arguments[2], value);
                cursor.updateCurrentRowFromMap(change);
                return;
            }
            var format = table.getFormat();
            System.out.printf("FORMAT\t%d\t%d\t%d\t%d%n", format.PAGE_SIZE,
                format.MAX_INLINE_LONG_VALUE_SIZE, format.MAX_LONG_VALUE_ROW_SIZE,
                format.SIZE_LONG_VALUE_DEF);
            map("GlobalFree", (UsageMap) field(table.getPageChannel(), "_globalUsageMap"));
            map("TableOwned", (UsageMap) field(table, "_ownedPages"));
            map("TableHint", (UsageMap) field(table, "_freeSpacePages"));
            Row row = cursor.getCurrentRow();
            System.out.printf("ROW\t%s%n", row.getId());
            for (String name : List.of("Memo", "OLE")) {
                Column column = table.getColumn(name);
                Object holder = field(column, "_lvalBufferH");
                map(name + "Owned", (UsageMap) field(holder, "_ownedPages"));
                map(name + "Hint", (UsageMap) field(holder, "_freeSpacePages"));
                Object stored = row.get(name);
                byte[] bytes = stored == null ? new byte[0]
                    : stored instanceof String ? stored.toString().getBytes(StandardCharsets.UTF_8)
                    : (byte[]) stored;
                System.out.printf("VALUE\t%s\t%s\t%d\t%s%n", name,
                    stored == null ? "NULL" : "PRESENT", bytes.length, hash(bytes));
            }
        }
    }
}
