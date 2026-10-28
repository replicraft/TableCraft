import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.DateTimeType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Index;
import com.healthmarketscience.jackcess.IndexBuilder;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import com.healthmarketscience.jackcess.impl.IndexImpl;
import com.healthmarketscience.jackcess.impl.IndexData;
import java.lang.reflect.Method;
import java.nio.channels.FileChannel;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.time.LocalDateTime;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.HexFormat;
import java.util.List;
import java.util.Set;
import java.util.TimeZone;

/** Finite public-writer recipe. Source eligibility is proved externally. */
public final class ScalarIndexInsertReference {
    private static final long MAX_BYTES = 64L * 1024 * 1024;
    private static final String TABLE = "Items";

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setEnforceForeignKeys(true);
        database.setTimeZone(TimeZone.getTimeZone("UTC"));
        database.setDateTimeType(DateTimeType.LOCAL_DATE_TIME);
        database.setLinkResolver((owner, token) -> {
            throw new AssertionError("No linked target authorized");
        });
    }

    private static DataType kind(String family) {
        return switch (family) {
            case "Boolean" -> DataType.BOOLEAN;
            case "Single" -> DataType.FLOAT;
            case "Double" -> DataType.DOUBLE;
            case "DateTime" -> DataType.SHORT_DATE_TIME;
            default -> throw new IllegalArgumentException("Closed family required");
        };
    }

    private static boolean ascending(String direction) {
        if (!List.of("ASC", "DESC").contains(direction)) {
            throw new IllegalArgumentException("Closed direction required");
        }
        return direction.equals("ASC");
    }

    private static List<Object[]> sourceRows(String family) {
        kind(family);
        List<Object[]> rows = new ArrayList<>();
        if (family.equals("Boolean")) {
            rows.add(new Object[] {1, false, new byte[0]});
            rows.add(new Object[] {2, true, new byte[0]});
            rows.add(new Object[] {3, false, new byte[0]});
            return rows;
        }
        int count = family.equals("Single") ? 400 : 277;
        for (int position = 0; position < count; position++) {
            Object value = switch (family) {
                case "Single" -> Float.intBitsToFloat(0x3f000000 + position);
                case "Double" -> Double.longBitsToDouble(0x3fe0000000000000L + position);
                case "DateTime" -> LocalDateTime.of(2000, 1, 1, 12, 0).plusDays(position);
                default -> throw new AssertionError("Closed family");
            };
            rows.add(new Object[] {position + 1, value, new byte[0]});
        }
        rows.add(new Object[] {count + 1, null, new byte[0]});
        return rows;
    }

    private static List<Object[]> requests(String family) {
        return switch (family) {
            case "Boolean" -> List.<Object[]>of(
                new Object[] {4, false, new byte[0]},
                new Object[] {5, true, new byte[0]});
            case "Single" -> List.<Object[]>of(
                new Object[] {402, Float.intBitsToFloat(0x80000000), new byte[0]},
                new Object[] {403, Float.intBitsToFloat(0), new byte[0]});
            case "Double" -> List.<Object[]>of(
                new Object[] {279, Double.longBitsToDouble(0x8000000000000000L), new byte[0]});
            case "DateTime" -> List.<Object[]>of(
                new Object[] {279, LocalDateTime.of(1899, 12, 30, 0, 0), new byte[0]});
            default -> throw new IllegalArgumentException("Closed family required");
        };
    }

    private static void declarations(Table table, String family, String direction) {
        boolean asc = ascending(direction);
        if (table.getColumns().size() != 3 || table.getIndexes().size() != 4
                || table.getColumn("ID").getType() != DataType.LONG
                || table.getColumn("ID").isAutoNumber()
                || table.getColumn("Group").getType() != kind(family)
                || table.getColumn("Group").isAutoNumber()
                || table.getColumn("Bytes").getType() != DataType.BINARY
                || table.getColumn("Bytes").getLength() != 255) {
            throw new AssertionError("Exact native declarations required");
        }
        Set<Integer> physical = new HashSet<>();
        int primary = -1;
        int secondary = -1;
        for (Index index : table.getIndexes()) {
            int number = ((IndexImpl) index).getIndexData().getIndexDataNumber();
            physical.add(number);
            if (index.getColumns().size() != 1 || index.shouldIgnoreNulls()) {
                throw new AssertionError("Exact single represented-null index required");
            }
            String name = index.getName();
            if (List.of("PrimaryID", "PrimaryAlias").contains(name)) {
                if (!index.isUnique() || !index.isRequired()
                        || !index.getColumns().get(0).isAscending()
                        || !index.getColumns().get(0).getName().equals("ID")
                        || index.isPrimaryKey() != name.equals("PrimaryID")) {
                    throw new AssertionError("Exact primary/alias roles required");
                }
                if (primary != -1 && primary != number) {
                    throw new AssertionError("Primary aliases must share physical data");
                }
                primary = number;
            } else if (List.of("GroupOrder", "GroupAlias").contains(name)) {
                if (index.isUnique() || index.isRequired() || index.isPrimaryKey()
                        || index.getColumns().get(0).isAscending() != asc
                        || !index.getColumns().get(0).getName().equals("Group")) {
                    throw new AssertionError("Exact secondary/alias roles required");
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
            throw new AssertionError("Exact two physical owners required");
        }
    }

    private static void encoder(Table table, String family) throws Exception {
        IndexImpl index = (IndexImpl) table.getIndex("GroupOrder");
        Method method = IndexData.class.getDeclaredMethod("createEntryBytes", Object[].class);
        method.setAccessible(true);
        for (String stage : List.of("OLD", "NEW")) {
            List<Object[]> rows = stage.equals("OLD") ? sourceRows(family) : requests(family);
            for (Object[] row : rows) {
                byte[] bytes = (byte[]) method.invoke(index.getIndexData(), (Object) row);
                System.out.printf("ENCODE\t%s\t%d\t%s%n", stage, (Integer) row[0],
                    HexFormat.of().formatHex(bytes));
            }
        }
    }

    private static void generate(Path path, String profile, String family, String direction)
            throws Exception {
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
                .addColumn(new ColumnBuilder("Group", kind(family)))
                .addColumn(new ColumnBuilder("Bytes", DataType.BINARY).setLength(255))
                .addIndex(new IndexBuilder("PrimaryID").addColumns(true, "ID").setPrimaryKey())
                .addIndex(new IndexBuilder("PrimaryAlias").addColumns(true, "ID")
                    .setUnique().setRequired())
                .addIndex(new IndexBuilder("GroupOrder").addColumns(ascending(direction), "Group"))
                .addIndex(new IndexBuilder("GroupAlias").addColumns(ascending(direction), "Group"))
                .toTable(database);
            for (Object[] row : sourceRows(family)) {
                table.addRow(row);
            }
            declarations(table, family, direction);
            if (table.getRowCount() != sourceRows(family).size()) {
                throw new AssertionError("Exact source cardinality required");
            }
            System.out.printf("SOURCE\t%s\t%s\t%s\t%d%n", profile, family, direction,
                table.getRowCount());
            encoder(table, family);
        }
    }

    private static void append(Path source, Path output, String family, String direction)
            throws Exception {
        if (Files.exists(output)) {
            throw new IllegalArgumentException("Fresh reference copy required");
        }
        Files.copy(source, output);
        try (BoundedFileChannel channel = new BoundedFileChannel(FileChannel.open(output,
                StandardOpenOption.READ, StandardOpenOption.WRITE), MAX_BYTES);
             Database database = new DatabaseBuilder(output).setChannel(channel).open()) {
            inert(database);
            Table table = database.getTable(TABLE);
            declarations(table, family, direction);
            if (table.getRowCount() != sourceRows(family).size()) {
                throw new AssertionError("Unchanged source cardinality required");
            }
            table.addRows(requests(family));
            if (table.getRowCount() != sourceRows(family).size() + requests(family).size()) {
                throw new AssertionError("Exact final cardinality required");
            }
            declarations(table, family, direction);
            System.out.printf("AFTER\t%s\t%s\t%d%n", family, direction, table.getRowCount());
        }
    }

    public static void main(String[] args) throws Exception {
        if (args.length == 5 && args[0].equals("generate")) {
            generate(Path.of(args[1]), args[2], args[3], args[4]);
        } else if (args.length == 5 && args[0].equals("append")) {
            append(Path.of(args[1]), Path.of(args[2]), args[3], args[4]);
        } else {
            throw new IllegalArgumentException("Closed generate/append arguments required");
        }
    }
}
