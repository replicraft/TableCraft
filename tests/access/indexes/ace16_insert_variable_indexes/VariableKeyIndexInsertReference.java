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
import java.nio.charset.StandardCharsets;
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

/** Closed public-writer recipe. Raw source eligibility is proved separately. */
public final class VariableKeyIndexInsertReference {
    private static final long MAX_BYTES = 64L * 1024 * 1024;
    private static final String TABLE = "Items";

    private static final class Recipe {
        final String family;
        final String shape;
        final String directions;
        final String mode;
        final String policy;

        Recipe(String family, String shape, String directions, String mode, String policy) {
            if (!List.of("Binary", "Legacy", "Modern").contains(family)
                    || !List.of("Single", "PayloadThenGroup", "GroupThenPayload").contains(shape)
                    || !List.of("fitted", "pressure", "digest", "digest-unique-guard").contains(mode)
                    || !List.of("ordinary", "unique", "unique-ignored").contains(policy)) {
                throw new IllegalArgumentException("Closed variable-key recipe required");
            }
            int components = shape.equals("Single") ? 1 : 2;
            if (directions.length() != components
                    || directions.chars().anyMatch(value -> value != 'A' && value != 'D')) {
                throw new IllegalArgumentException("Exact directions required");
            }
            boolean valid;
            if (mode.equals("fitted")) {
                valid = shape.equals("Single") ? policy.equals("ordinary")
                    : (policy.equals("ordinary") && directions.equals("AD"))
                        || (policy.equals("unique-ignored") && shape.equals("PayloadThenGroup")
                            && directions.equals("DA"));
            } else if (mode.equals("pressure")) {
                valid = family.equals("Binary") && shape.equals("Single") && policy.equals("ordinary");
            } else {
                valid = family.equals("Modern") && shape.equals("Single")
                    && policy.equals(mode.equals("digest") ? "ordinary" : "unique");
            }
            if (!valid) {
                throw new IllegalArgumentException("Case outside fixed inventory");
            }
            this.family = family;
            this.shape = shape;
            this.directions = directions;
            this.mode = mode;
            this.policy = policy;
        }

        List<String> columns() {
            return switch (shape) {
                case "Single" -> List.of("Payload");
                case "PayloadThenGroup" -> List.of("Payload", "Group");
                case "GroupThenPayload" -> List.of("Group", "Payload");
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

    private static byte[] repeated(int value, int count) {
        byte[] result = new byte[count];
        java.util.Arrays.fill(result, (byte) value);
        return result;
    }

    private static Object[] row(Recipe recipe, int id, Integer group, byte[] payload) {
        Object value = payload;
        if (!recipe.family.equals("Binary") && payload != null) {
            for (byte item : payload) {
                if (item != 'a' && item != 'z') {
                    throw new IllegalArgumentException("Closed finite text alphabet required");
                }
            }
            value = new String(payload, StandardCharsets.US_ASCII);
        }
        return new Object[] {Integer.valueOf(id), group, value};
    }

    private static byte[] ascii(String text) {
        return text.getBytes(StandardCharsets.US_ASCII);
    }

    private static List<Object[]> sourceRows(Recipe recipe) {
        List<Object[]> rows = new ArrayList<>();
        if (recipe.mode.equals("pressure")) {
            for (int position = 1; position <= 12; position++) {
                rows.add(row(recipe, position, null, repeated(position, 255)));
            }
            rows.add(row(recipe, 13, null, null));
        } else if (recipe.mode.startsWith("digest")) {
            rows.add(row(recipe, 1, null, repeated('a', 255)));
        } else if (recipe.ignored()) {
            rows.add(row(recipe, 1, null, null));
            rows.add(row(recipe, 2, null, ascii("a")));
            rows.add(row(recipe, 3, 10, null));
            rows.add(row(recipe, 4, null, ascii("")));
            rows.add(row(recipe, 5, 20, ascii("aaa")));
        } else {
            rows.add(row(recipe, 1, null, null));
            rows.add(row(recipe, 2, 10, ascii("")));
            rows.add(row(recipe, 3, null, ascii("a")));
            rows.add(row(recipe, 4, 20, ascii("aaa")));
            rows.add(row(recipe, 5, null, ascii("z")));
        }
        return rows;
    }

    private static List<Object[]> requests(Recipe recipe) {
        if (recipe.mode.equals("pressure")) {
            return List.<Object[]>of(row(recipe, 14, null, repeated(255, 255)),
                row(recipe, 15, null, repeated(254, 255)),
                row(recipe, 16, null, repeated(253, 255)));
        }
        if (recipe.mode.equals("digest")) {
            return List.<Object[]>of(row(recipe, 10, null, repeated('a', 255)),
                                    row(recipe, 11, null, repeated('z', 255)));
        }
        if (recipe.mode.equals("digest-unique-guard")) {
            return List.<Object[]>of(row(recipe, 10, null, ascii("aa")));
        }
        if (recipe.ignored()) {
            return List.<Object[]>of(row(recipe, 10, null, null), row(recipe, 11, 11, null),
                row(recipe, 12, null, ascii("aa")), row(recipe, 13, 11, ascii("")),
                row(recipe, 14, null, null));
        }
        byte[] maximum = repeated(recipe.family.equals("Binary") ? 255 : 'z', 255);
        return List.<Object[]>of(row(recipe, 10, null, ascii("aa")), row(recipe, 11, 20, null),
            row(recipe, 12, null, null), row(recipe, 13, null, ascii("")),
            row(recipe, 14, 30, maximum));
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

    private static void declarations(Table table, Recipe recipe) throws java.io.IOException {
        if (table.getColumns().size() != 3 || table.getIndexes().size() != 4
                || table.getColumn("ID").getType() != DataType.LONG
                || table.getColumn("ID").isAutoNumber()
                || table.getColumn("Group").getType() != DataType.LONG
                || table.getColumn("Group").isAutoNumber()
                || table.getColumn("Payload").getType()
                    != (recipe.family.equals("Binary") ? DataType.BINARY : DataType.TEXT)
                || table.getColumn("Payload").getLength()
                    != (recipe.family.equals("Binary") ? 255 : 510)
                || table.getColumn("Payload").isCompressedUnicode()) {
            throw new AssertionError("Exact manual variable declarations required");
        }
        if (!recipe.family.equals("Binary")) {
            var expected = recipe.family.equals("Legacy")
                ? ColumnImpl.GENERAL_LEGACY_SORT_ORDER : ColumnImpl.GENERAL_SORT_ORDER;
            var actual = ((ColumnImpl) table.getColumn("Payload")).getTextSortOrder();
            if (actual.getValue() != expected.getValue()
                    || actual.getVariant() != expected.getVariant()
                    || actual.getVersion() != expected.getVersion()
                    || !Boolean.TRUE.equals(table.getColumn("Payload").getProperties()
                        .getValue("AllowZeroLength"))) {
                throw new AssertionError("Exact sort family and explicit empty-text policy required");
            }
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
            ColumnBuilder payload = recipe.family.equals("Binary")
                ? new ColumnBuilder("Payload", DataType.BINARY).setLength(255)
                : new ColumnBuilder("Payload", DataType.TEXT).setLengthInUnits(255)
                    .setCompressedUnicode(false).putProperty("AllowZeroLength", true);
            if (!recipe.family.equals("Binary")) {
                payload.setTextSortOrder(recipe.family.equals("Legacy")
                    ? ColumnImpl.GENERAL_LEGACY_SORT_ORDER : ColumnImpl.GENERAL_SORT_ORDER);
            }
            Table table = new TableBuilder(TABLE)
                .addColumn(new ColumnBuilder("ID", DataType.LONG).setAutoNumber(false)
                    .putProperty("Required", true))
                .addColumn(new ColumnBuilder("Group", DataType.LONG).setAutoNumber(false))
                .addColumn(payload)
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
            System.out.printf("SOURCE\t%s\t%s\t%s\t%s\t%s\t%s\t%d%n", profile,
                recipe.family, recipe.shape, recipe.directions, recipe.mode, recipe.policy, table.getRowCount());
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
            System.out.printf("AFTER\t%s\t%s\t%s\t%s\t%s\t%d%n", recipe.family, recipe.shape,
                recipe.directions, recipe.mode, recipe.policy, table.getRowCount());
        }
    }

    public static void main(String[] args) throws Exception {
        if (args.length == 8 && args[0].equals("generate")) {
            generate(Path.of(args[1]), args[2], new Recipe(args[3], args[4], args[5], args[6], args[7]));
        } else if (args.length == 8 && args[0].equals("append")) {
            append(Path.of(args[1]), Path.of(args[2]), new Recipe(args[3], args[4], args[5], args[6], args[7]));
        } else {
            throw new IllegalArgumentException("Closed generate/append arguments required");
        }
    }
}
