import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.impl.IndexData;
import com.healthmarketscience.jackcess.impl.PageChannel;
import com.healthmarketscience.jackcess.impl.TableImpl;
import com.healthmarketscience.jackcess.impl.UsageMap;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashSet;
import java.util.List;
import java.util.BitSet;
import java.util.Set;

/** Original read-only, pinned-JAR introspection; never invokes a writer method. */
public final class DeepUsageMapProbe {
    private static Object field(Object object, Class<?> owner, String name) throws Exception {
        Field field = owner.getDeclaredField(name);
        field.setAccessible(true);
        return field.get(object);
    }

    private static int number(Object object, Class<?> owner, String name) throws Exception {
        Method method = owner.getDeclaredMethod(name);
        method.setAccessible(true);
        return ((Number) method.invoke(object)).intValue();
    }

    private static void map(String name, UsageMap map, int pageCount) throws Exception {
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
        boolean global = name.equals("JackcessGlobalFree");
        for (int page = 0; page < pageCount; page++) {
            boolean present = page >= base && bits.get(page - base);
            if (!global && map.containsPageNumber(page) != present) {
                throw new AssertionError("Protected bitmap disagrees with public membership");
            }
            if (present) {
                System.out.printf("MEMBER\t%s\t%d%n", name, page);
            }
        }
    }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length != 1) {
            throw new IllegalArgumentException("Expected one input path");
        }
        Path path = Path.of(arguments[0]);
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            database.setLinkResolver((source, token) -> {
                throw new AssertionError("No linked targets authorized");
            });
            TableImpl tree = (TableImpl) database.getTable("Deep");
            var format = tree.getFormat();
            int pageCount = Math.toIntExact(Files.size(path) / format.PAGE_SIZE);
            System.out.printf("FILE\t%d\t%d%n", format.PAGE_SIZE, pageCount);
            System.out.printf("FORMAT\t%d\t%d\t%d\t%d\t%d\t%d\t%d\t%d\t%d%n",
                format.OFFSET_NUM_COLS, format.OFFSET_NUM_INDEX_SLOTS,
                format.OFFSET_NUM_INDEXES, format.OFFSET_INDEX_DEF_BLOCK,
                format.SIZE_INDEX_DEFINITION, format.SIZE_COLUMN_HEADER,
                format.OFFSET_OWNED_PAGES, format.OFFSET_FREE_SPACE_PAGES,
                format.OFFSET_ROW_START);
            UsageMap global = (UsageMap) field(tree.getPageChannel(), PageChannel.class, "_globalUsageMap");
            map("JackcessGlobalFree", global, pageCount);
            for (String name : List.of("Deep", "Preserved")) {
                TableImpl table = (TableImpl) database.getTable(name);
                map(name + "Owned", (UsageMap) field(table, TableImpl.class, "_ownedPages"), pageCount);
                map(name + "FreeSpace", (UsageMap) field(table, TableImpl.class, "_freeSpacePages"), pageCount);
            }
            Set<Integer> seen = new HashSet<>();
            for (var index : tree.getIndexes()) {
                IndexData data = index.getIndexData();
                data.getEntryCount();
                System.out.printf("INDEX\t%s\t%d\t%d\t%d\t%d%n", index.getName(),
                    data.getIndexDataNumber(), number(data, IndexData.class, "getRootPageNumber"),
                    data.getOwnedPageCount(), data.getEntryCount());
                if (seen.add(data.getIndexDataNumber())) {
                    UsageMap owned = (UsageMap) field(data, IndexData.class, "_ownedPages");
                    map("Index" + data.getIndexDataNumber() + "Owned", owned, pageCount);
                }
            }
        }
    }
}
