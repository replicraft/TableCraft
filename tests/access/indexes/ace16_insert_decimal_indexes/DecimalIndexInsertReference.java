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
import java.math.BigDecimal;
import java.math.BigInteger;
import java.math.RoundingMode;
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
public final class DecimalIndexInsertReference {
    private static final long MAX_BYTES = 64L * 1024 * 1024;
    private static final String TABLE = "Items";

    private static final class Recipe {
        final String shape;
        final String directions;
        final String mode;
        final String policy;
        final int precision;
        final int scale;

        Recipe(String shape, String directions, String mode, String policy,
               int precision, int scale) {
            if (!List.of("Decimal", "DecimalThenGuid", "GuidThenDecimal",
                         "DecimalThenDecimal", "ReversedDecimals").contains(shape)
                    || !List.of("pressure", "fitted").contains(mode)
                    || !List.of("ordinary", "unique", "unique-ignored").contains(policy)) {
                throw new IllegalArgumentException("Closed Decimal recipe required");
            }
            int components = shape.equals("Decimal") ? 1 : 2;
            if (directions.length() != components
                    || directions.chars().anyMatch(value -> value != 'A' && value != 'D')
                    || !List.of("1/0", "3/2", "28/4", "28/28")
                        .contains(precision + "/" + scale)) {
                throw new IllegalArgumentException("Exact directions and declared domain required");
            }
            boolean twoDecimals = shape.equals("DecimalThenDecimal")
                || shape.equals("ReversedDecimals");
            if (twoDecimals) {
                if (!mode.equals("fitted") || !directions.equals("AD")
                        || precision != 28 || scale != 4) {
                    throw new IllegalArgumentException("Closed two-domain cases required");
                }
            } else if (!policy.equals("ordinary")
                    || (shape.equals("Decimal") && mode.equals("pressure")
                        && (precision != 28 || scale != 4))
                    || (!shape.equals("Decimal") && (!mode.equals("pressure")
                        || precision != 28 || scale != 4))) {
                throw new IllegalArgumentException("Closed ordinary source recipe required");
            }
            this.shape = shape;
            this.directions = directions;
            this.mode = mode;
            this.policy = policy;
            this.precision = precision;
            this.scale = scale;
        }

        boolean twoDecimals() {
            return shape.equals("DecimalThenDecimal") || shape.equals("ReversedDecimals");
        }

        List<String> columns() {
            return switch (shape) {
                case "Decimal" -> List.of("Amount");
                case "DecimalThenGuid", "DecimalThenDecimal" -> List.of("Amount", "Peer");
                case "GuidThenDecimal", "ReversedDecimals" -> List.of("Peer", "Amount");
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

    private static BigDecimal decimal(BigInteger coefficient, int precision, int scale) {
        if (coefficient == null) {
            return null;
        }
        if (coefficient.abs().compareTo(BigInteger.TEN.pow(precision)) >= 0) {
            throw new IllegalArgumentException("Exact declared precision exceeded");
        }
        return new BigDecimal(coefficient, scale).setScale(scale, RoundingMode.UNNECESSARY);
    }

    private static Object[] row(Recipe recipe, int id, BigInteger amount, Object peer) {
        Object actualPeer = recipe.twoDecimals()
            ? decimal((BigInteger) peer, 3, 2) : peer;
        return new Object[] {Integer.valueOf(id),
                            decimal(amount, recipe.precision, recipe.scale), actualPeer};
    }

    private static BigInteger integer(long value) {
        return BigInteger.valueOf(value);
    }

    private static List<Object[]> sourceRows(Recipe recipe) {
        List<Object[]> rows = new ArrayList<>();
        if (recipe.mode.equals("pressure")) {
            int count = recipe.shape.equals("Decimal") ? 164 : 88;
            for (int position = 0; position < count; position++) {
                rows.add(row(recipe, position + 1, integer(position - count / 2),
                             recipe.shape.equals("Decimal") ? null : guid(position)));
            }
            rows.add(row(recipe, count + 1, null, null));
        } else {
            BigInteger limit = BigInteger.TEN.pow(recipe.precision).subtract(BigInteger.ONE);
            if (recipe.shape.equals("Decimal")) {
                BigInteger[] values = {limit.negate(), integer(-1), integer(0),
                                       integer(1), limit, null};
                for (int position = 0; position < values.length; position++) {
                    rows.add(row(recipe, position + 1, values[position], null));
                }
            } else {
                rows.add(row(recipe, 1, limit, integer(-999)));
                rows.add(row(recipe, 2, limit, integer(999)));
                rows.add(row(recipe, 3, null, integer(0)));
                rows.add(row(recipe, 4, integer(0), null));
                rows.add(row(recipe, 5, null, null));
                rows.add(row(recipe, 6, limit.negate(), integer(-1)));
            }
        }
        return rows;
    }

    private static List<Object[]> requests(Recipe recipe) {
        if (recipe.mode.equals("pressure")) {
            int count = recipe.shape.equals("Decimal") ? 164 : 88;
            return List.<Object[]>of(row(recipe, count + 2, integer(count),
                recipe.shape.equals("Decimal") ? null : guid(count)));
        }
        BigInteger limit = BigInteger.TEN.pow(recipe.precision).subtract(BigInteger.ONE);
        if (recipe.shape.equals("Decimal")) {
            return List.<Object[]>of(row(recipe, 7, limit.negate(), null),
                row(recipe, 8, limit, null), row(recipe, 9, integer(0), null),
                row(recipe, 10, null, null), row(recipe, 11, null, null));
        }
        if (!recipe.unique()) {
            return List.<Object[]>of(row(recipe, 7, limit, integer(-999)),
                row(recipe, 8, null, integer(0)), row(recipe, 9, integer(0), null),
                row(recipe, 10, null, null), row(recipe, 11, null, null));
        }
        return List.<Object[]>of(row(recipe, 7, integer(1), integer(1)),
            row(recipe, 8, null, integer(1)), row(recipe, 9, integer(-1), null),
            row(recipe, 10, null, null), row(recipe, 11, null, null));
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
                || table.getColumn("Amount").getType() != DataType.NUMERIC
                || table.getColumn("Amount").getPrecision() != recipe.precision
                || table.getColumn("Amount").getScale() != recipe.scale
                || table.getColumn("Amount").isAutoNumber()
                || table.getColumn("Peer").getType()
                    != (recipe.twoDecimals() ? DataType.NUMERIC : DataType.GUID)
                || table.getColumn("Peer").isAutoNumber()) {
            throw new AssertionError("Exact manual per-field Decimal declarations required");
        }
        if (recipe.twoDecimals() && (table.getColumn("Peer").getPrecision() != 3
                || table.getColumn("Peer").getScale() != 2)) {
            throw new AssertionError("Independent Peer precision/scale required");
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
                if (row[1] != null && (!(row[1] instanceof BigDecimal value)
                        || value.scale() != recipe.scale)) {
                    throw new AssertionError("Amount exact BigDecimal scale required");
                }
                if (recipe.twoDecimals() && row[2] != null
                        && (!(row[2] instanceof BigDecimal value) || value.scale() != 2)) {
                    throw new AssertionError("Peer exact BigDecimal scale required");
                }
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
            ColumnBuilder peer = recipe.twoDecimals()
                ? new ColumnBuilder("Peer", DataType.NUMERIC).setPrecision(3).setScale(2)
                : new ColumnBuilder("Peer", DataType.GUID).setAutoNumber(false);
            Table table = new TableBuilder(TABLE)
                .addColumn(new ColumnBuilder("ID", DataType.LONG).setAutoNumber(false)
                    .putProperty("Required", true))
                .addColumn(new ColumnBuilder("Amount", DataType.NUMERIC)
                    .setPrecision(recipe.precision).setScale(recipe.scale))
                .addColumn(peer)
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
        if (args.length == 9 && args[0].equals("generate")) {
            generate(Path.of(args[1]), args[2], new Recipe(args[3], args[4], args[5], args[6],
                Integer.parseInt(args[7]), Integer.parseInt(args[8])));
        } else if (args.length == 9 && args[0].equals("append")) {
            append(Path.of(args[1]), Path.of(args[2]), new Recipe(args[3], args[4], args[5], args[6],
                Integer.parseInt(args[7]), Integer.parseInt(args[8])));
        } else {
            throw new IllegalArgumentException("Closed generate/append arguments required");
        }
    }
}
