import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Index;
import com.healthmarketscience.jackcess.IndexBuilder;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import com.healthmarketscience.jackcess.impl.IndexImpl;
import java.nio.channels.FileChannel;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.TimeZone;

/** Unexecuted finite source/reference recipe; raw gates remain external. */
public final class IgnoredIndexInsertReference {
    private static final long MAX_BYTES = 64L * 1024 * 1024;
    private static final String TABLE = "Items";

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setEnforceForeignKeys(true);
        database.setTimeZone(TimeZone.getTimeZone("UTC"));
        database.setLinkResolver((owner, token) -> {
            throw new AssertionError("No linked target authorized");
        });
    }

    private static String variant(String value) {
        if (!List.of("mixed", "empty", "empty-first-nonnull", "all-null-node", "both-root-pressure").contains(value)) {
            throw new IllegalArgumentException("Closed variant required");
        }
        return value;
    }

    private static int sourceCount(String value) {
        return switch (variant(value)) {
            case "all-null-node" -> 500;
            case "both-root-pressure" -> 400;
            default -> 3;
        };
    }

    private static void declarations(Table table) {
        if (table.getColumns().size() != 3 || table.getIndexes().size() != 4
                || table.getColumn("ID").getType() != DataType.LONG
                || table.getColumn("ID").isAutoNumber()
                || table.getColumn("Group").getType() != DataType.LONG
                || table.getColumn("Group").isAutoNumber()
                || table.getColumn("Score").getType() != DataType.BINARY
                || table.getColumn("Score").getLength() != 255) {
            throw new AssertionError("Exact native declarations required");
        }
        Set<Integer> physical = new HashSet<>();
        int primary = -1;
        int secondary = -1;
        for (Index index : table.getIndexes()) {
            int number = ((IndexImpl) index).getIndexData().getIndexDataNumber();
            physical.add(number);
            if (index.getColumns().size() != 1
                    || !index.getColumns().get(0).isAscending()) {
                throw new AssertionError("Exact single ASC key columns required");
            }
            String name = index.getName();
            if (List.of("PrimaryID", "PrimaryAlias").contains(name)) {
                if (!index.isUnique() || !index.isRequired() || index.shouldIgnoreNulls()
                        || !index.getColumns().get(0).getName().equals("ID")
                        || index.isPrimaryKey() != name.equals("PrimaryID")) {
                    throw new AssertionError("Exact primary/alias roles required");
                }
                if (primary != -1 && primary != number) {
                    throw new AssertionError("Primary alias did not share physical data");
                }
                primary = number;
            } else if (List.of("GroupOrder", "GroupAlias").contains(name)) {
                if (index.isUnique() || index.isRequired() || index.isPrimaryKey()
                        || !index.shouldIgnoreNulls()
                        || !index.getColumns().get(0).getName().equals("Group")) {
                    throw new AssertionError("Exact nullable ordinary secondary required");
                }
                if (secondary != -1 && secondary != number) {
                    throw new AssertionError("Secondary alias did not share physical data");
                }
                secondary = number;
            } else {
                throw new AssertionError("Unexpected logical index");
            }
        }
        if (physical.size() != 2 || primary < 0 || secondary < 0 || primary == secondary) {
            throw new AssertionError("Exactly two physical owners required");
        }
    }

    private static void generate(Path path, String profile, String variant) throws Exception {
        variant(variant);
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
                .addColumn(new ColumnBuilder("Group", DataType.LONG).setAutoNumber(false))
                .addColumn(new ColumnBuilder("Score", DataType.BINARY).setLength(255))
                .addIndex(new IndexBuilder("PrimaryID").addColumns(true, "ID").setPrimaryKey())
                .addIndex(new IndexBuilder("PrimaryAlias").addColumns(true, "ID")
                    .setUnique().setRequired())
                .addIndex(new IndexBuilder("GroupOrder").addColumns(true, "Group").setIgnoreNulls())
                .addIndex(new IndexBuilder("GroupAlias").addColumns(true, "Group").setIgnoreNulls())
                .toTable(database);
            if (variant.equals("both-root-pressure")) {
                for (int id = -200; id <= 200; id++) {
                    if (id != 0) {
                        table.addRow(id, id == -200 ? null : id, new byte[0]);
                    }
                }
            } else if (variant.equals("all-null-node")) {
                for (int id = 1; id <= 500; id++) {
                    table.addRow(id, id - 251, new byte[0]);
                }
            } else {
                boolean empty = variant.equals("empty");
                if (variant.equals("empty-first-nonnull")) {
                    throw new IllegalArgumentException("First nonnull uses an immutable empty source copy");
                }
                table.addRow(1, empty ? null : 10, null);
                table.addRow(2, empty ? null : 10, new byte[0]);
                table.addRow(3, null, new byte[] {(byte) 0x80, 0, (byte) 0xff});
            }
            declarations(table);
            if (table.getRowCount() != sourceCount(variant)) {
                throw new AssertionError("Exact source cardinality required");
            }
            System.out.printf("SOURCE\t%s\t%s\t%d%n", profile, variant, table.getRowCount());
        }
    }

    private static void append(Path source, Path output, String variant) throws Exception {
        variant(variant);
        if (Files.exists(output)) {
            throw new IllegalArgumentException("Fresh reference copy required");
        }
        Files.copy(source, output);
        try (BoundedFileChannel channel = new BoundedFileChannel(FileChannel.open(output,
                StandardOpenOption.READ, StandardOpenOption.WRITE), MAX_BYTES);
             Database database = new DatabaseBuilder(output).setChannel(channel).open()) {
            inert(database);
            Table table = database.getTable(TABLE);
            declarations(table);
            if (table.getRowCount() != sourceCount(variant)) {
                throw new AssertionError("Unchanged exact source count required");
            }
            List<Object[]> requests = new ArrayList<>();
            if (variant.equals("both-root-pressure")) {
                for (int id = 201; id <= 254; id++) {
                    requests.add(new Object[] {id, id, new byte[0]});
                }
                requests.add(new Object[] {255, null, new byte[0]});
                requests.add(new Object[] {256, null, new byte[0]});
            } else if (variant.equals("all-null-node")) {
                requests.add(new Object[] {501, null, new byte[0]});
                requests.add(new Object[] {502, null, new byte[0]});
            } else if (variant.equals("empty") || variant.equals("empty-first-nonnull")) {
                requests.add(new Object[] {4, variant.equals("empty") ? null : 30, new byte[0]});
                requests.add(new Object[] {5, null, new byte[0]});
            } else {
                requests.add(new Object[] {4, 10, new byte[0]});
                requests.add(new Object[] {5, 20, new byte[0]});
                requests.add(new Object[] {6, 20, new byte[0]});
                requests.add(new Object[] {7, null, new byte[0]});
            }
            table.addRows(requests);
            if (table.getRowCount() != (sourceCount(variant) + requests.size())) {
                throw new AssertionError("Exact final cardinality required");
            }
            declarations(table);
            System.out.printf("AFTER\t%s\t%d%n", variant, table.getRowCount());
        }
    }

    public static void main(String[] args) throws Exception {
        if (args.length == 4 && args[0].equals("generate")) {
            generate(Path.of(args[1]), args[2], args[3]);
        } else if (args.length == 4 && args[0].equals("append")) {
            append(Path.of(args[1]), Path.of(args[2]), args[3]);
        } else {
            throw new IllegalArgumentException("Closed generate/append arguments required");
        }
    }
}
