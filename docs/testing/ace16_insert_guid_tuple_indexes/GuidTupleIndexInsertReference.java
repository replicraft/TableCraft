import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.DateTimeType;
import com.healthmarketscience.jackcess.Index;
import com.healthmarketscience.jackcess.IndexBuilder;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import com.healthmarketscience.jackcess.impl.IndexData;
import com.healthmarketscience.jackcess.impl.IndexImpl;
import java.lang.reflect.Method;
import java.nio.channels.FileChannel;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.HexFormat;
import java.util.List;
import java.util.Set;
import java.util.TimeZone;
import java.util.UUID;

/** Closed public-writer recipe. Raw source eligibility is proved separately. */
public final class GuidTupleIndexInsertReference {
    private static final long MAX_BYTES = 64L * 1024 * 1024;
    private static final String TABLE = "Items";

    private static final class Recipe {
        final String shape;
        final String directions;
        final String mode;
        final String policy;

        Recipe(String shape, String directions, String mode, String policy) {
            if (!List.of("Guid", "GuidThenScore", "ScoreThenGuid").contains(shape)
                    || !List.of("pressure", "fitted").contains(mode)
                    || !List.of("ordinary", "unique", "unique-ignored").contains(policy)) {
                throw new IllegalArgumentException("Closed recipe required");
            }
            int components = shape.equals("Guid") ? 1 : 2;
            if (directions.length() != components
                    || directions.chars().anyMatch(value -> value != 'A' && value != 'D')) {
                throw new IllegalArgumentException("Exact component directions required");
            }
            if (mode.equals("pressure") && !policy.equals("ordinary")) {
                throw new IllegalArgumentException("Pressure uses represented NULLs");
            }
            if (mode.equals("fitted") && shape.equals("Guid")) {
                throw new IllegalArgumentException("Fitted policy corpus uses tuples");
            }
            this.shape = shape;
            this.directions = directions;
            this.mode = mode;
            this.policy = policy;
        }

        List<String> columns() {
            return switch (shape) {
                case "Guid" -> List.of("Group");
                case "GuidThenScore" -> List.of("Group", "Score");
                case "ScoreThenGuid" -> List.of("Score", "Group");
                default -> throw new AssertionError("Closed shape");
            };
        }

        boolean unique() {
            return !policy.equals("ordinary");
        }

        boolean ignored() {
            return policy.equals("unique-ignored");
        }
    }

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setEnforceForeignKeys(true);
        database.setTimeZone(TimeZone.getTimeZone("UTC"));
        database.setDateTimeType(DateTimeType.LOCAL_DATE_TIME);
        database.setLinkResolver((owner, token) -> {
            throw new AssertionError("No linked target authorized");
        });
    }

    private static String guid(int firstGroup) {
        if (firstGroup < 0) {
            throw new IllegalArgumentException("Nonnegative closed GUID seed required");
        }
        UUID uuid = new UUID(((long) firstGroup << 32) | 0x04050607L,
                             0x80090a0b0c0d0effL);
        return ColumnImpl.toGUIDString(uuid);
    }

    private static List<Object[]> sourceRows(Recipe recipe) {
        List<Object[]> rows = new ArrayList<>();
        if (recipe.mode.equals("pressure")) {
            int count = recipe.shape.equals("Guid") ? 157 : 128;
            for (int position = 0; position < count; position++) {
                rows.add(new Object[] {position + 1, guid(position), position - 64});
            }
            Object score = recipe.shape.equals("Guid") ? Integer.valueOf(12345) : null;
            rows.add(new Object[] {count + 1, null, score});
        } else {
            rows.add(new Object[] {1, guid(0), 10});
            rows.add(new Object[] {2, guid(0), 20});
            rows.add(new Object[] {3, null, 30});
            rows.add(new Object[] {4, guid(2), null});
            rows.add(new Object[] {5, null, null});
            rows.add(new Object[] {6, ColumnImpl.toGUIDString(new UUID(0L, 0L)), 0});
        }
        return rows;
    }

    private static List<Object[]> requests(Recipe recipe) {
        if (recipe.mode.equals("pressure")) {
            int count = recipe.shape.equals("Guid") ? 157 : 128;
            return List.<Object[]>of(new Object[] {count + 2, guid(count), count - 64});
        }
        if (!recipe.unique()) {
            return List.<Object[]>of(
                new Object[] {7, guid(0), 10},
                new Object[] {8, null, 30},
                new Object[] {9, guid(2), null},
                new Object[] {10, null, null},
                new Object[] {11, null, null});
        }
        return List.<Object[]>of(
            new Object[] {7, guid(4), 40},
            new Object[] {8, null, 31},
            new Object[] {9, guid(3), null},
            new Object[] {10, null, null},
            new Object[] {11, null, null});
    }

    private static IndexBuilder secondary(String name, Recipe recipe) {
        IndexBuilder index = new IndexBuilder(name);
        List<String> columns = recipe.columns();
        for (int position = 0; position < columns.size(); position++) {
            index.addColumns(recipe.directions.charAt(position) == 'A', columns.get(position));
        }
        if (recipe.unique()) {
            index.setUnique();
        }
        if (recipe.ignored()) {
            index.setIgnoreNulls();
        }
        return index;
    }

    private static void declarations(Table table, Recipe recipe) {
        if (table.getColumns().size() != 3 || table.getIndexes().size() != 4
                || table.getColumn("ID").getType() != DataType.LONG
                || table.getColumn("ID").isAutoNumber()
                || table.getColumn("Group").getType() != DataType.GUID
                || table.getColumn("Group").getLength() != 16
                || table.getColumn("Group").isAutoNumber()
                || table.getColumn("Score").getType() != DataType.LONG
                || table.getColumn("Score").isAutoNumber()) {
            throw new AssertionError("Exact manual fixed declarations required");
        }
        Set<Integer> physical = new HashSet<>();
        int primary = -1;
        int secondary = -1;
        for (Index index : table.getIndexes()) {
            int number = ((IndexImpl) index).getIndexData().getIndexDataNumber();
            physical.add(number);
            String name = index.getName();
            if (List.of("PrimaryID", "PrimaryAlias").contains(name)) {
                if (!index.isUnique() || !index.isRequired() || index.shouldIgnoreNulls()
                        || index.isPrimaryKey() != name.equals("PrimaryID")
                        || index.getColumns().size() != 1
                        || !index.getColumns().get(0).getName().equals("ID")
                        || !index.getColumns().get(0).isAscending()) {
                    throw new AssertionError("Exact primary roles required");
                }
                if (primary != -1 && primary != number) {
                    throw new AssertionError("Primary aliases must share physical data");
                }
                primary = number;
            } else if (List.of("GroupOrder", "GroupAlias").contains(name)) {
                if (index.isUnique() != recipe.unique() || index.isRequired()
                        || index.shouldIgnoreNulls() != recipe.ignored() || index.isPrimaryKey()
                        || index.getColumns().size() != recipe.columns().size()) {
                    throw new AssertionError("Exact secondary roles required");
                }
                for (int position = 0; position < recipe.columns().size(); position++) {
                    if (!index.getColumns().get(position).getName().equals(recipe.columns().get(position))
                            || index.getColumns().get(position).isAscending()
                                != (recipe.directions.charAt(position) == 'A')) {
                        throw new AssertionError("Written component order and directions required");
                    }
                }
                if (secondary != -1 && secondary != number) {
                    throw new AssertionError("Secondary aliases must share physical data");
                }
                secondary = number;
            } else {
                throw new AssertionError("Unexpected logical index");
            }
        }
        if (physical.size() != 2 || primary != 0 || secondary != 1) {
            throw new AssertionError("Exactly two ordered physical owners required");
        }
    }

    private static void encoder(Table table, Recipe recipe) throws Exception {
        IndexImpl index = (IndexImpl) table.getIndex("GroupOrder");
        Method method = IndexData.class.getDeclaredMethod("createEntryBytes", Object[].class);
        method.setAccessible(true);
        for (String stage : List.of("OLD", "NEW")) {
            List<Object[]> rows = stage.equals("OLD") ? sourceRows(recipe) : requests(recipe);
            for (Object[] row : rows) {
                byte[] bytes = (byte[]) method.invoke(index.getIndexData(), (Object) row);
                System.out.printf("ENCODE\t%s\t%d\t%s%n", stage, (Integer) row[0],
                    HexFormat.of().formatHex(bytes));
            }
        }
    }

    private static void generate(Path path, String profile, Recipe recipe) throws Exception {
        if (Files.exists(path) || !List.of("V2016", "V2019").contains(profile)) {
            throw new IllegalArgumentException("Fresh source and closed profile required");
        }
        try (BoundedFileChannel channel = new BoundedFileChannel(FileChannel.open(path,
                StandardOpenOption.CREATE_NEW, StandardOpenOption.READ,
                StandardOpenOption.WRITE), MAX_BYTES);
             Database database = new DatabaseBuilder(path).setChannel(channel)
                .setFileFormat(Database.FileFormat.valueOf(profile)).setAutoSync(false).create()) {
            inert(database);
            Table table = new TableBuilder(TABLE)
                .addColumn(new ColumnBuilder("ID", DataType.LONG).setAutoNumber(false)
                    .putProperty("Required", true))
                .addColumn(new ColumnBuilder("Group", DataType.GUID).setAutoNumber(false))
                .addColumn(new ColumnBuilder("Score", DataType.LONG).setAutoNumber(false))
                .addIndex(new IndexBuilder("PrimaryID").addColumns(true, "ID").setPrimaryKey())
                .addIndex(new IndexBuilder("PrimaryAlias").addColumns(true, "ID")
                    .setUnique().setRequired())
                .addIndex(secondary("GroupOrder", recipe))
                .addIndex(secondary("GroupAlias", recipe))
                .toTable(database);
            for (Object[] row : sourceRows(recipe)) {
                table.addRow(row);
            }
            declarations(table, recipe);
            if (table.getRowCount() != sourceRows(recipe).size()) {
                throw new AssertionError("Exact source count required");
            }
            System.out.printf("SOURCE\t%s\t%s\t%s\t%s\t%s\t%d%n", profile,
                recipe.shape, recipe.directions, recipe.mode, recipe.policy, table.getRowCount());
            encoder(table, recipe);
        }
    }

    private static void append(Path source, Path output, Recipe recipe) throws Exception {
        if (Files.exists(output)) {
            throw new IllegalArgumentException("Fresh reference copy required");
        }
        Files.copy(source, output);
        try (BoundedFileChannel channel = new BoundedFileChannel(FileChannel.open(output,
                StandardOpenOption.READ, StandardOpenOption.WRITE), MAX_BYTES);
             Database database = new DatabaseBuilder(output).setChannel(channel).open()) {
            inert(database);
            Table table = database.getTable(TABLE);
            declarations(table, recipe);
            if (table.getRowCount() != sourceRows(recipe).size()) {
                throw new AssertionError("Unchanged source count required");
            }
            table.addRows(requests(recipe));
            if (table.getRowCount() != sourceRows(recipe).size() + requests(recipe).size()) {
                throw new AssertionError("Exact final count required");
            }
            declarations(table, recipe);
            System.out.printf("AFTER\t%s\t%s\t%s\t%s\t%d%n", recipe.shape,
                recipe.directions, recipe.mode, recipe.policy, table.getRowCount());
        }
    }

    public static void main(String[] args) throws Exception {
        if (args.length == 7 && args[0].equals("generate")) {
            generate(Path.of(args[1]), args[2], new Recipe(args[3], args[4], args[5], args[6]));
        } else if (args.length == 7 && args[0].equals("append")) {
            append(Path.of(args[1]), Path.of(args[2]), new Recipe(args[3], args[4], args[5], args[6]));
        } else {
            throw new IllegalArgumentException("Closed generate/append arguments required");
        }
    }
}
