import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.Cursor;
import com.healthmarketscience.jackcess.CursorBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.IndexBuilder;
import com.healthmarketscience.jackcess.TableBuilder;
import com.healthmarketscience.jackcess.impl.IndexData;
import com.healthmarketscience.jackcess.impl.PageChannel;
import com.healthmarketscience.jackcess.impl.RowIdImpl;
import com.healthmarketscience.jackcess.impl.TableImpl;
import com.healthmarketscience.jackcess.impl.UsageMap;
import java.io.IOException;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.channels.FileChannel;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.BitSet;
import java.util.HashSet;
import java.util.HexFormat;
import java.util.List;
import java.util.Set;
import java.util.TimeZone;

/** New bounded height-two/non-root-parent experiment; no raw database writes. */
public final class InternalParentSplitFixture {
    private static final long MAX_BYTES = 64L * 1024 * 1024;
    private static final int MAX_GENERATED_ROWS = 150_000;
    private static final int SENTINEL_ROWS = 1;
    private static final int MAX_ROWS = MAX_GENERATED_ROWS - SENTINEL_ROWS;
    private static final int MAX_FINAL_ROWS = 16_384;
    private static final int MAX_GOAL_ROWS = 1536;
    private static final int BATCH_ROWS = 32;
    private static final int MAX_DEPTH = 16;
    private static final int PAGE_BYTES = 4096;
    private static final int HEADER_BYTES = 27;
    private static final int ENTRY_START = 480;
    private static final int CAPACITY = PAGE_BYTES - ENTRY_START;
    private static final int KIND_OFFSET = 0;
    private static final int VERSION_OFFSET = 1;
    private static final int FREE_OFFSET = 2;
    private static final int OWNER_OFFSET = 4;
    private static final int STAMP_OFFSET = 8;
    private static final int PREVIOUS_OFFSET = 12;
    private static final int NEXT_OFFSET = 16;
    private static final int TAIL_OFFSET = 20;
    private static final int PREFIX_OFFSET = 24;
    private static final int HEIGHT_OFFSET = 26;
    private static final int LEAF = 4;
    private static final int NODE = 3;
    private static final int KEY_ROW_BYTES = 9;
    private static final int CHILD_BYTES = 4;
    private static final int SIGN_XOR = 0x80000000;
    private static final String TABLE = "SparseInt32";
    private static final String INDEX = "PrimaryKey";

    private record Entry(byte[] keyRow, int child) {}
    private record Page(int number, int height, int previous, int next, int tail,
                        List<Entry> entries) {}
    private record Gate(int root, int parent, int leaf, int selected,
                        int leafBytes, int parentBytes, int leftCount, int leafCount,
                        List<Page> path) {}

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setEnforceForeignKeys(true);
        database.setTimeZone(TimeZone.getTimeZone("UTC"));
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
        Method method = owner.getDeclaredMethod(name);
        method.setAccessible(true);
        return ((Number) method.invoke(object)).intValue();
    }

    private static String hash(Path path) throws Exception {
        MessageDigest digest = MessageDigest.getInstance("SHA-256");
        try (var input = Files.newInputStream(path)) {
            byte[] buffer = new byte[64 * 1024];
            int count;
            while ((count = input.read(buffer)) != -1) {
                digest.update(buffer, 0, count);
            }
        }
        return HexFormat.of().formatHex(digest.digest());
    }

    private static int unsigned16(byte[] data, int offset) {
        return Short.toUnsignedInt(ByteBuffer.wrap(data, offset, Short.BYTES)
            .order(ByteOrder.LITTLE_ENDIAN).getShort());
    }

    private static int little32(byte[] data, int offset) {
        return ByteBuffer.wrap(data, offset, Integer.BYTES)
            .order(ByteOrder.LITTLE_ENDIAN).getInt();
    }

    private static int big32(byte[] data, int offset) {
        return ByteBuffer.wrap(data, offset, Integer.BYTES).getInt();
    }

    private static int key(byte[] keyRow) {
        if (keyRow.length != KEY_ROW_BYTES || keyRow[0] != 0x7f) {
            throw new AssertionError("Expected non-NULL ascending native Int32");
        }
        return big32(keyRow, 1) ^ SIGN_XOR;
    }

    private static Page page(FileChannel channel, int number, int owner) throws Exception {
        ByteBuffer buffer = ByteBuffer.allocate(PAGE_BYTES);
        long offset = (long) number * PAGE_BYTES;
        while (buffer.hasRemaining()) {
            int count = channel.read(buffer, offset + buffer.position());
            if (count <= 0) {
                throw new AssertionError("Incomplete raw page");
            }
        }
        byte[] raw = buffer.array();
        int kind = Byte.toUnsignedInt(raw[KIND_OFFSET]);
        int height = Byte.toUnsignedInt(raw[HEIGHT_OFFSET]);
        if ((kind != LEAF && kind != NODE) || raw[VERSION_OFFSET] != 1
                || little32(raw, OWNER_OFFSET) != owner || little32(raw, STAMP_OFFSET) != 0
                || (kind == LEAF) != (height == 0)) {
            throw new AssertionError("Unqualified Int32 tree header");
        }
        int prefix = unsigned16(raw, PREFIX_OFFSET);
        if (prefix > KEY_ROW_BYTES) {
            throw new AssertionError("Unqualified shared key/ROW prefix");
        }
        List<Entry> entries = new ArrayList<>();
        byte[] first = null;
        int previous = 0;
        for (int end = 1; end <= CAPACITY; end++) {
            if ((raw[HEADER_BYTES + end / Byte.SIZE] & (1 << (end % Byte.SIZE))) == 0) {
                continue;
            }
            byte[] suffix = Arrays.copyOfRange(raw, ENTRY_START + previous, ENTRY_START + end);
            byte[] full;
            if (first == null) {
                full = suffix;
                first = full;
            } else {
                full = new byte[prefix + suffix.length];
                System.arraycopy(first, 0, full, 0, prefix);
                System.arraycopy(suffix, 0, full, prefix, suffix.length);
            }
            int width = KEY_ROW_BYTES + (kind == NODE ? CHILD_BYTES : 0);
            if (full.length != width) {
                throw new AssertionError("Unexpected native Int32 entry width");
            }
            byte[] keyRow = Arrays.copyOf(full, KEY_ROW_BYTES);
            key(keyRow);
            int child = kind == NODE ? big32(full, KEY_ROW_BYTES) : 0;
            if (kind == NODE && child <= 0) {
                throw new AssertionError("Invalid child identity");
            }
            entries.add(new Entry(keyRow, child));
            previous = end;
        }
        if (entries.isEmpty() || unsigned16(raw, FREE_OFFSET) != CAPACITY - previous) {
            throw new AssertionError("Invalid raw entry/free geometry");
        }
        return new Page(number, height, little32(raw, PREVIOUS_OFFSET),
            little32(raw, NEXT_OFFSET), little32(raw, TAIL_OFFSET), entries);
    }

    private static int bestBytes(List<byte[]> entries, boolean node) {
        if (entries.isEmpty()) {
            return 0;
        }
        int prefix = KEY_ROW_BYTES;
        byte[] first = entries.get(0);
        for (byte[] entry : entries) {
            int length = 0;
            while (length < prefix && first[length] == entry[length]) {
                length++;
            }
            prefix = length;
        }
        return entries.size() * (KEY_ROW_BYTES + (node ? CHILD_BYTES : 0))
            - prefix * (entries.size() - 1);
    }

    private static Gate gate(Path path, TableImpl table) throws Exception {
        IndexData data = table.getIndex(INDEX).getIndexData();
        int root = number(data, IndexData.class, "getRootPageNumber");
        List<Page> pages = new ArrayList<>();
        Set<Integer> visited = new HashSet<>();
        try (FileChannel channel = FileChannel.open(path, StandardOpenOption.READ)) {
            int next = root;
            while (true) {
                if (pages.size() >= MAX_DEPTH || !visited.add(next)) {
                    throw new AssertionError("Unqualified path depth/cycle");
                }
                Page current = page(channel, next, table.getTableDefPageNumber());
                if (!pages.isEmpty() && current.height() + 1 != pages.get(pages.size() - 1).height()) {
                    throw new AssertionError("Inconsistent subtree heights");
                }
                pages.add(current);
                if (current.height() == 0) {
                    break;
                }
                next = current.tail() != 0 ? current.tail()
                    : current.entries().get(current.entries().size() - 1).child();
            }
        }
        if (pages.size() != 3 || pages.get(0).height() != 2) {
            return null;
        }
        Page leaf = pages.get(pages.size() - 1);
        Page parent = pages.get(pages.size() - 2);
        if (parent.number() == root || parent.height() != 1) {
            throw new AssertionError("Expected a distinct height-one internal parent");
        }
        System.out.printf("INTERNAL_PATH\t%d\t%d\t%d\t%d\t%d\t%d%n",
            table.getRowCount(), root, parent.number(), leaf.number(),
            parent.entries().size(), leaf.entries().size());
        List<byte[]> changed = new ArrayList<>();
        for (Entry entry : leaf.entries()) {
            changed.add(entry.keyRow());
        }
        byte[] replacement = changed.get(changed.size() - 1).clone();
        int selected = key(replacement);
        if (selected != table.getRowCount() || selected == Integer.MAX_VALUE) {
            throw new AssertionError("Unexpected selected maximum");
        }
        Arrays.fill(replacement, 1, 1 + Integer.BYTES, (byte) 0xff);
        changed.set(changed.size() - 1, replacement);
        int leafBytes = bestBytes(changed, false);
        int middle = changed.size() / 2;
        if (leafBytes <= CAPACITY || middle == 0
                || bestBytes(changed.subList(0, middle), false) > CAPACITY
                || bestBytes(changed.subList(middle, changed.size()), false) > CAPACITY) {
            return null;
        }
        List<byte[]> bounds = new ArrayList<>();
        for (Entry entry : parent.entries()) {
            bounds.add(entry.keyRow());
        }
        if (parent.tail() == 0) {
            if (parent.entries().get(parent.entries().size() - 1).child() != leaf.number()) {
                throw new AssertionError("Bounded last child differs");
            }
            bounds.remove(bounds.size() - 1);
        } else if (parent.tail() != leaf.number()) {
            throw new AssertionError("Tail child differs");
        }
        bounds.add(changed.get(middle - 1));
        if (parent.tail() == 0) {
            bounds.add(replacement);
        }
        int parentBytes = bestBytes(bounds, true);
        if (parentBytes <= CAPACITY) {
            return null;
        }
        return new Gate(root, parent.number(), leaf.number(), selected, leafBytes,
            parentBytes, middle, leaf.entries().size(), pages);
    }

    private static byte[] collect(FileChannel channel, int owner, int number,
                                  List<Page> leaves, Set<Integer> visited, int depth) throws Exception {
        if (depth >= MAX_DEPTH || !visited.add(number)) {
            throw new AssertionError("Invalid full tree depth/cycle");
        }
        Page current = page(channel, number, owner);
        if (current.height() == 0) {
            leaves.add(current);
            return current.entries().get(current.entries().size() - 1).keyRow();
        }
        byte[] maximum = null;
        for (Entry entry : current.entries()) {
            maximum = collect(channel, owner, entry.child(), leaves, visited, depth + 1);
            if (!Arrays.equals(maximum, entry.keyRow())) {
                throw new AssertionError("Full key/ROW child maximum differs");
            }
        }
        if (current.tail() != 0) {
            maximum = collect(channel, owner, current.tail(), leaves, visited, depth + 1);
        }
        return maximum;
    }

    private static void generate(Path path, String profile, Path selection) throws Exception {
        if (Files.exists(path) || Files.exists(selection)
                || !List.of("V2016", "V2019").contains(profile)) {
            throw new IllegalArgumentException("Expected fresh outputs and explicit ACE16 profile");
        }
        Gate selected = null;
        List<Page> leaves = new ArrayList<>();
        int rows = 0;
        try (BoundedFileChannel channel = new BoundedFileChannel(FileChannel.open(path,
                StandardOpenOption.CREATE_NEW, StandardOpenOption.READ, StandardOpenOption.WRITE), MAX_BYTES);
             Database database = new DatabaseBuilder(path).setChannel(channel)
                .setFileFormat(Database.FileFormat.valueOf(profile)).setAutoSync(false).create()) {
            inert(database);
            TableImpl table = (TableImpl) new TableBuilder(TABLE)
                .addColumn(new ColumnBuilder("Key", DataType.LONG))
                .addColumn(new ColumnBuilder("Marker", DataType.LONG))
                .addIndex(new IndexBuilder(INDEX).addColumns("Key").setPrimaryKey())
                .toTable(database);
            new TableBuilder("Sentinel")
                .addColumn(new ColumnBuilder("Key", DataType.LONG))
                .addColumn(new ColumnBuilder("Marker", DataType.LONG))
                .toTable(database).addRow(91, 12345);
            for (int identity = 1; identity <= MAX_ROWS; identity++) {
                table.addRow(identity, identity * 3);
                rows = identity;
                if (identity % BATCH_ROWS != 0 && identity != MAX_ROWS) {
                    continue;
                }
                database.flush();
                if (Files.size(path) > MAX_BYTES) {
                    throw new AssertionError("Bounded channel did not enforce file ceiling");
                }
                selected = gate(path, table);
                System.out.printf("CHECKPOINT\t%d\t%d\t%s%n", rows,
                    table.getIndex(INDEX).getIndexData().getOwnedPageCount(), selected != null);
                if (selected != null) {
                    try (FileChannel raw = FileChannel.open(path, StandardOpenOption.READ)) {
                        collect(raw, table.getTableDefPageNumber(), selected.root(), leaves,
                            new HashSet<>(), 0);
                    }
                    break;
                }
            }
            database.flush();
        }
        if (selected == null) {
            throw new IllegalStateException("No qualifying parent overflow within the frozen generation bounds");
        }
        Set<Integer> keep = new HashSet<>();
        for (int index = 0; index < leaves.size(); index++) {
            Page leaf = leaves.get(index);
            int previous = index == 0 ? 0 : leaves.get(index - 1).number();
            int next = index + 1 == leaves.size() ? 0 : leaves.get(index + 1).number();
            if (leaf.previous() != previous || leaf.next() != next) {
                throw new AssertionError("Raw leaf sibling order differs");
            }
            if (leaf.number() == selected.leaf()) {
                for (Entry entry : leaf.entries()) {
                    keep.add(key(entry.keyRow()));
                }
            } else {
                keep.add(key(leaf.entries().get(leaf.entries().size() - 1).keyRow()));
            }
        }
        if (keep.size() > MAX_FINAL_ROWS) {
            throw new IllegalStateException("First qualifying geometry exceeds existing live-row ceiling");
        }
        List<String> lines = new ArrayList<>();
        lines.add("SOURCE_SHA256\t" + hash(path));
        lines.add("INITIAL_ROWS\t" + rows);
        lines.add("GOAL_MET\t" + (keep.size() <= MAX_GOAL_ROWS));
        lines.add("ROOT\t" + selected.root());
        lines.add("ROOT_HEIGHT\t2");
        lines.add("PARENT_IS_ROOT\tfalse");
        lines.add("PARENT\t" + selected.parent());
        lines.add("LEAF\t" + selected.leaf());
        lines.add("SELECTED\t" + selected.selected());
        lines.add("LEAF_BEST_AFTER\t" + selected.leafBytes());
        lines.add("PARENT_BEST_AFTER\t" + selected.parentBytes());
        lines.add("LEFT_COUNT\t" + selected.leftCount());
        lines.add("TARGET_COUNT\t" + selected.leafCount());
        for (Page leaf : leaves) {
            lines.add("ORIGINAL_LEAF\t" + leaf.number() + "\t" + leaf.entries().size()
                + "\t" + key(leaf.entries().get(leaf.entries().size() - 1).keyRow()));
        }
        for (int identity : keep.stream().sorted().toList()) {
            lines.add("KEEP\t" + identity);
        }
        Files.write(selection, lines, StandardOpenOption.CREATE_NEW);
        System.out.printf("QUALIFIED_GENERATION\t%d\t%d\t%d%n", rows, keep.size(), Files.size(path));
    }

    private static void sparse(Path source, Path output, Path selection) throws Exception {
        if (Files.exists(output) || Files.size(source) > MAX_BYTES) {
            throw new IllegalArgumentException("Expected bounded original and fresh sparse copy");
        }
        List<String> lines = Files.readAllLines(selection);
        Set<Integer> keep = new HashSet<>();
        String expectedHash = null;
        for (String line : lines) {
            String[] fields = line.split("\t");
            if (fields[0].equals("SOURCE_SHA256")) {
                expectedHash = fields[1];
            } else if (fields[0].equals("KEEP") && !keep.add(Integer.parseInt(fields[1]))) {
                throw new AssertionError("Duplicate keep selector");
            }
        }
        if (!hash(source).equals(expectedHash) || keep.isEmpty() || keep.size() > MAX_FINAL_ROWS) {
            throw new AssertionError("Sparse source/selector identity differs");
        }
        Files.copy(source, output);
        try (BoundedFileChannel channel = new BoundedFileChannel(FileChannel.open(output,
                StandardOpenOption.READ, StandardOpenOption.WRITE), MAX_BYTES);
             Database database = new DatabaseBuilder(output).setChannel(channel).setAutoSync(false).open()) {
            inert(database);
            var table = database.getTable(TABLE);
            Cursor cursor = CursorBuilder.createCursor(table);
            int removed = 0;
            while (cursor.moveToNextRow()) {
                int identity = cursor.getCurrentRow().getInt("Key");
                if (!keep.contains(identity)) {
                    cursor.deleteCurrentRow();
                    removed++;
                }
            }
            if (table.getRowCount() != keep.size()) {
                throw new AssertionError("Sparse live population differs");
            }
            database.flush();
            System.out.printf("SPARSE\t%d\t%d%n", removed, table.getRowCount());
        }
        if (!hash(source).equals(expectedHash)) {
            throw new AssertionError("Larger source changed during sparse-copy actions");
        }
    }

    private static void map(String name, UsageMap map, int pages) throws Exception {
        System.out.printf("MAP\t%s\t%d\t%d\t%d\t%d\t%d\t%d%n", name,
            number(map, UsageMap.class, "getTablePageNumber"),
            number(map, UsageMap.class, "getRowStart"), number(map, UsageMap.class, "getRowEnd"),
            number(map, UsageMap.class, "getStartPage"), number(map, UsageMap.class, "getEndPage"),
            map.getPageCount());
        Method getter = UsageMap.class.getDeclaredMethod("getPageNumbers");
        getter.setAccessible(true);
        BitSet bits = (BitSet) getter.invoke(map);
        int base = number(map, UsageMap.class, "getStartPage");
        for (int page = 0; page < pages; page++) {
            if (page >= base && bits.get(page - base)) {
                System.out.printf("MEMBER\t%s\t%d%n", name, page);
            }
        }
    }

    private static void describe(Path source) throws Exception {
        try (Database database = new DatabaseBuilder(source).setReadOnly(true).open()) {
            inert(database);
            if (!database.getRelationships().isEmpty()) {
                throw new AssertionError("Unexpected relationship");
            }
            for (String name : List.of(TABLE, "Sentinel")) {
                TableImpl table = (TableImpl) database.getTable(name);
                if (table.getProperties().iterator().hasNext()) {
                    throw new AssertionError("Unexpected table properties");
                }
                System.out.printf("TABLE\t%s\t%d\t%d%n", name,
                    table.getRowCount(), table.getTableDefPageNumber());
                for (var column : table.getColumns()) {
                    if (column.getType() != DataType.LONG
                            || column.getProperties().iterator().hasNext()) {
                        throw new AssertionError("Unexpected field carrier/properties");
                    }
                    System.out.printf("COLUMN\t%s\t%s\t%s\t%d\t%d%n", name,
                        column.getName(), column.getType(), column.getColumnIndex(), column.getLength());
                }
                int ordinal = 0;
                for (var row : table) {
                    RowIdImpl location = (RowIdImpl) row.getId();
                    System.out.printf("ROW\t%s\t%d\t%d\t%d\t%d\t%d%n", name, ordinal,
                        row.getInt("Key"), row.getInt("Marker"),
                        location.getPageNumber(), location.getRowNumber());
                    ordinal++;
                }
                for (var index : table.getIndexes()) {
                    IndexData data = index.getIndexData();
                    System.out.printf("INDEX\t%s\t%s\t%d\t%d\t%d\t%d\t%s\t%s\t%s\t%s%n",
                        name, index.getName(), data.getIndexDataNumber(),
                        number(data, IndexData.class, "getRootPageNumber"),
                        index.getUniqueEntryCount(), index.getUniqueEntryCountOffset(),
                        index.isPrimaryKey(), index.isRequired(), index.isUnique(), index.shouldIgnoreNulls());
                    for (var column : index.getColumns()) {
                        System.out.printf("INDEX_COLUMN\t%s\t%s\t%s\t%s%n", name,
                            index.getName(), column.getName(), column.isAscending());
                    }
                    Cursor cursor = CursorBuilder.createCursor(index);
                    while (cursor.moveToNextRow()) {
                        System.out.printf("ENTRY\t%s\t%s\t%d%n", name,
                            index.getName(), cursor.getCurrentRow().getInt("Key"));
                    }
                }
            }
            TableImpl table = (TableImpl) database.getTable(TABLE);
            int pages = Math.toIntExact(Files.size(source) / PAGE_BYTES);
            System.out.printf("FILE\t%d\t%d%n", PAGE_BYTES, pages);
            map("JackcessGlobalFree", (UsageMap) field(table.getPageChannel(),
                PageChannel.class, "_globalUsageMap"), pages);
            map("TableOwned", (UsageMap) field(table, TableImpl.class, "_ownedPages"), pages);
            map("TableFreeSpace", (UsageMap) field(table, TableImpl.class, "_freeSpacePages"), pages);
            IndexData data = table.getIndex(INDEX).getIndexData();
            map("IndexOwned", (UsageMap) field(data, IndexData.class, "_ownedPages"), pages);
        }
    }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length == 4 && arguments[0].equals("generate")) {
            generate(Path.of(arguments[1]), arguments[2], Path.of(arguments[3]));
        } else if (arguments.length == 4 && arguments[0].equals("sparse")) {
            sparse(Path.of(arguments[1]), Path.of(arguments[2]), Path.of(arguments[3]));
        } else if (arguments.length == 2 && arguments[0].equals("describe")) {
            describe(Path.of(arguments[1]));
        } else {
            throw new IllegalArgumentException("Expected generate OUTPUT PROFILE SELECTION, sparse SOURCE OUTPUT SELECTION, or describe SOURCE");
        }
    }
}
