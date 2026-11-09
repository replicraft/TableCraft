import com.healthmarketscience.jackcess.Cursor;
import com.healthmarketscience.jackcess.CursorBuilder;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.impl.RowIdImpl;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.impl.PageChannel;
import com.healthmarketscience.jackcess.impl.TableImpl;
import com.healthmarketscience.jackcess.impl.UsageMap;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.BitSet;
import java.util.List;

/** Read-only metadata probe; no page allocation or mutation calls. */
public final class SlotMapProbe {
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
                throw new AssertionError("Raw/public map membership differs");
            }
            if (present) {
                System.out.printf("MEMBER\t%s\t%d%n", name, page);
            }
        }
    }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length != 2 && arguments.length != 3) {
            throw new IllegalArgumentException("Expected database, table and optional ID");
        }
        Path path = Path.of(arguments[0]);
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            database.setLinkResolver((source, token) -> {
                throw new AssertionError("No linked target authorized");
            });
            TableImpl table = (TableImpl) database.getTable(arguments[1]);
            int width = table.getFormat().PAGE_SIZE;
            int pages = Math.toIntExact(Files.size(path) / width);
            System.out.printf("FILE\t%d\t%d%n", width, pages);
            System.out.printf("FORMAT\t%d\t%d\t%d%n", table.getFormat().OFFSET_FREE_SPACE,
                table.getFormat().OFFSET_OWNED_PAGES, table.getFormat().OFFSET_FREE_SPACE_PAGES);
            map("JackcessGlobalFree", (UsageMap) field(table.getPageChannel(),
                PageChannel.class, "_globalUsageMap"), pages);
            map("TableOwned", (UsageMap) field(table, TableImpl.class, "_ownedPages"), pages);
            map("TableFreeSpace", (UsageMap) field(table, TableImpl.class, "_freeSpacePages"), pages);
            if (arguments.length == 3) {
                int identity = Integer.parseInt(arguments[2]);
                Cursor cursor = CursorBuilder.createCursor(table);
                boolean found = false;
                while (cursor.moveToNextRow()) {
                    Row row = cursor.getCurrentRow(List.of("ID"));
                    if (row.getInt("ID") != identity) {
                        continue;
                    }
                    RowIdImpl header = (RowIdImpl) row.getId();
                    var state = table.createRowState();
                    TableImpl.positionAtRowData(state, header);
                    RowIdImpl target = state.getFinalRowId();
                    System.out.printf("LOCATION\t%d\t%d\t%d\t%d%n", header.getPageNumber(),
                        header.getRowNumber(), target.getPageNumber(), target.getRowNumber());
                    found = true;
                    break;
                }
                if (!found) {
                    throw new AssertionError("Requested ID absent");
                }
            }
        }
    }
}
