import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.impl.IndexData;
import com.healthmarketscience.jackcess.impl.PageChannel;
import com.healthmarketscience.jackcess.impl.TableImpl;
import com.healthmarketscience.jackcess.impl.UsageMap;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.BitSet;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.TimeZone;

/** Fresh-copy action and read-only metadata; no fixture generation or stored code. */
public final class NonrootSplitReference {
    private static final int EXPECTED = 1000;
    private static final int REPLACEMENT = Integer.MAX_VALUE;
    private static final int ROWS = 1000;
    private static final int SELECTED_ORDINAL = 27;
    private static final List<String> TABLES = List.of("Tree", "Unindexed", "Sentinel");

    private static void inert(Database database) {
        database.setTimeZone(TimeZone.getTimeZone("UTC"));
        database.setEvaluateExpressions(false);
        database.setEnforceForeignKeys(true);
        database.setLinkResolver((source, token) -> {
            throw new AssertionError("No linked target is authorized");
        });
    }

    private static Object field(Object object, Class<?> owner, String name) throws Exception {
        Field member = owner.getDeclaredField(name);
        member.setAccessible(true);
        return member.get(object);
    }

    private static int number(Object object, Class<?> owner, String name) throws Exception {
        Method getter = owner.getDeclaredMethod(name);
        getter.setAccessible(true);
        return ((Number) getter.invoke(object)).intValue();
    }

    private static void map(String name, UsageMap map, int pages) throws Exception {
        System.out.printf("MAP\t%s\t%d\t%d\t%d\t%d\t%d\t%d%n", name,
            number(map, UsageMap.class, "getTablePageNumber"),
            number(map, UsageMap.class, "getRowStart"),
            number(map, UsageMap.class, "getRowEnd"),
            number(map, UsageMap.class, "getStartPage"),
            number(map, UsageMap.class, "getEndPage"), map.getPageCount());
        Method getter = UsageMap.class.getDeclaredMethod("getPageNumbers");
        getter.setAccessible(true);
        BitSet bits = (BitSet) getter.invoke(map);
        int base = number(map, UsageMap.class, "getStartPage");
        for (int page = 0; page < pages; page++) {
            boolean present = page >= base && bits.get(page - base);
            if (!name.equals("JackcessGlobalFree") && map.containsPageNumber(page) != present) {
                throw new AssertionError("Protected/public map membership differs");
            }
            if (present) {
                System.out.printf("MEMBER\t%s\t%d%n", name, page);
            }
        }
    }

    private static void maps(Path path) throws Exception {
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            inert(database);
            TableImpl first = (TableImpl) database.getTable("Tree");
            int width = first.getFormat().PAGE_SIZE;
            int pages = Math.toIntExact(Files.size(path) / width);
            System.out.printf("FILE\t%d\t%d%n", width, pages);
            UsageMap global = (UsageMap) field(first.getPageChannel(), PageChannel.class, "_globalUsageMap");
            map("JackcessGlobalFree", global, pages);
            for (String name : TABLES) {
                TableImpl table = (TableImpl) database.getTable(name);
                map(name + "Owned", (UsageMap) field(table, TableImpl.class, "_ownedPages"), pages);
                map(name + "FreeSpace", (UsageMap) field(table, TableImpl.class, "_freeSpacePages"), pages);
                Set<Integer> seen = new HashSet<>();
                for (var index : table.getIndexes()) {
                    IndexData data = index.getIndexData();
                    int entries = data.getEntryCount();
                    System.out.printf("INDEX\t%s/%s\t%d\t%d\t%d\t%d%n", name,
                        index.getName(), data.getIndexDataNumber(),
                        number(data, IndexData.class, "getRootPageNumber"),
                        data.getOwnedPageCount(), entries);
                    if (seen.add(data.getIndexDataNumber())) {
                        map("Index" + name + data.getIndexDataNumber() + "Owned",
                            (UsageMap) field(data, IndexData.class, "_ownedPages"), pages);
                    }
                }
            }
        }
    }

    private static void update(Path source, Path output) throws Exception {
        if (Files.exists(output)
                || source.toRealPath().equals(output.toAbsolutePath().normalize())) {
            throw new IllegalArgumentException("Expected a fresh, distinct output");
        }
        Files.copy(source, output);
        try (Database database = new DatabaseBuilder(output).open()) {
            inert(database);
            if (!database.isEnforceForeignKeys() || !database.getRelationships().isEmpty()) {
                throw new AssertionError("Expected enforced host setting and no relationships");
            }
            for (String name : TABLES) {
                Table table = database.getTable(name);
                if (table.getProperties().iterator().hasNext()) {
                    throw new AssertionError("Unexpected table properties");
                }
                for (var column : table.getColumns()) {
                    if (column.getProperties().iterator().hasNext()) {
                        throw new AssertionError("Unexpected column properties");
                    }
                }
            }
            Table table = database.getTable("Tree");
            var primary = table.getIndex("PrimaryKey");
            if (table.getRowCount() != ROWS || !primary.isPrimaryKey()
                    || !primary.isUnique() || !primary.isRequired()) {
                throw new AssertionError("Expected complete required-primary source");
            }
            Row selected = null;
            int ordinal = 0;
            for (Row row : table) {
                int identity = row.getInt("ID");
                if (identity == REPLACEMENT) {
                    throw new AssertionError("Replacement must be unused");
                }
                if (identity == EXPECTED) {
                    if (selected != null || ordinal != SELECTED_ORDINAL) {
                        throw new AssertionError("Selected live ordinal differs");
                    }
                    selected = row;
                }
                ordinal++;
            }
            if (selected == null || ordinal != ROWS) {
                throw new AssertionError("Selected identity/count differs");
            }
            selected.put("ID", REPLACEMENT);
            table.updateRow(selected);
            System.out.println("updated=1; foreignKeys=true; expressions=false; timezone=UTC");
        }
    }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length == 2 && arguments[0].equals("maps")) {
            maps(Path.of(arguments[1]));
        } else if (arguments.length == 3 && arguments[0].equals("update")) {
            update(Path.of(arguments[1]), Path.of(arguments[2]));
        } else {
            throw new IllegalArgumentException("Expected maps SOURCE or update SOURCE FRESH_OUTPUT");
        }
    }
}
