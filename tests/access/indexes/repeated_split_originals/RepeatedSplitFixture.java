import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.IndexBuilder;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import java.math.BigDecimal;
import java.nio.channels.FileChannel;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.Arrays;
import java.util.List;
import java.util.TimeZone;

/** Own finite root-leaf recipe; public builders only, no database byte edits. */
public final class RepeatedSplitFixture {
    private static final long MAX_BYTES = 64L * 1024 * 1024;
    private static final int MAX_ROWS = 4096;
    private static final int PRESERVED_BYTES = 32 * 1024;
    private static final List<String> TARGETS = List.of(
        "Int64Keys", "CurrencyKeys");
    private static final int[] TARGET_ROWS = {900, 900};
    private static final int PRESERVED_ROWS = 1;
    private static final int TARGET_MARKER_FACTOR = 17;
    private static final int EXPECTED_ROWS = Arrays.stream(TARGET_ROWS).sum()
        + PRESERVED_ROWS;
    private static int inserted;

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setEnforceForeignKeys(true);
        database.setTimeZone(TimeZone.getTimeZone("UTC"));
        database.setLinkResolver((source, token) -> {
            throw new AssertionError("No linked target is authorized");
        });
    }

    private static Object value(DataType type, long units) {
        return switch (type) {
            case BYTE -> {
                if (units < 0 || units > 255) {
                    throw new IllegalArgumentException("Unsigned Byte out of range");
                }
                yield (byte) units;
            }
            case INT -> {
                if (units < Short.MIN_VALUE || units > Short.MAX_VALUE) {
                    throw new IllegalArgumentException("Int16 out of range");
                }
                yield (short) units;
            }
            case BIG_INT -> units;
            case MONEY -> BigDecimal.valueOf(units, 4);
            default -> throw new IllegalArgumentException("Closed integral source type required");
        };
    }

    private static void append(Table table, Object... values) throws Exception {
        if (inserted >= MAX_ROWS) {
            throw new IllegalStateException("Frozen insertion ceiling reached");
        }
        table.addRow(values);
        inserted++;
    }

    private static Table table(Database database, String name, DataType type) throws Exception {
        TableBuilder builder = new TableBuilder(name)
            .addColumn(new ColumnBuilder("ID", DataType.LONG).putProperty("Required", true))
            .addColumn(new ColumnBuilder("KeyValue", type).putProperty("Required", true))
            .addColumn(new ColumnBuilder("Marker", DataType.LONG))
            .addIndex(new IndexBuilder("PrimaryID").addColumns("ID").setPrimaryKey());
        for (String label : List.of("Ascending", "Descending", "Alias")) {
            IndexBuilder index = new IndexBuilder(label)
                .addColumns(!label.equals("Descending"), "KeyValue").setRequired();
            builder.addIndex(index);
        }
        return builder.toTable(database);
    }

    private static void generate(Path path, String profile) throws Exception {
        if (Files.exists(path) || !List.of("V2016", "V2019").contains(profile)) {
            throw new IllegalArgumentException("Fresh output and explicit ACE16 format required");
        }
        inserted = 0;
        try (BoundedFileChannel channel = new BoundedFileChannel(FileChannel.open(path,
                StandardOpenOption.CREATE_NEW, StandardOpenOption.READ, StandardOpenOption.WRITE), MAX_BYTES);
             Database database = new DatabaseBuilder(path).setChannel(channel)
                .setFileFormat(Database.FileFormat.valueOf(profile)).setAutoSync(false).create()) {
            inert(database);
            DataType[] types = {DataType.BIG_INT, DataType.MONEY};
            for (int position = 0; position < TARGETS.size(); position++) {
                DataType type = types[position];
                String name = TARGETS.get(position);
                int count = TARGET_ROWS[position];
                Table target = table(database, name, type);
                for (int identity = 1; identity <= count; identity++) {
                    append(target, identity, value(type, 0), identity * TARGET_MARKER_FACTOR);
                }
                database.flush();
                System.out.printf("GENERATED_TABLE\t%s\t%d\t%d%n", name, count, inserted);
            }
            Table preserved = new TableBuilder("Preserved")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("Notes", DataType.MEMO))
                .addColumn(new ColumnBuilder("Payload", DataType.OLE)).toTable(database);
            byte[] payload = new byte[PRESERVED_BYTES];
            for (int position = 0; position < payload.length; position++) {
                payload[position] = (byte) (position * 29 + 7);
            }
            append(preserved, 1, "Preserved Unicode é🦀 ".repeat(1024), payload);
            database.flush();
        }
        if (inserted != EXPECTED_ROWS || Files.size(path) > MAX_BYTES) {
            throw new AssertionError("Unexpected final source population or file size");
        }
        System.out.printf("GENERATED\t%s\t%d\t%d%n", profile, inserted, Files.size(path));
    }

    private static void describe(Path path) throws Exception {
        System.out.println("BEGIN_GETTERS");
        SlotUpdateFixture.main(new String[] {"oracle", path.toString()});
        System.out.println("END_GETTERS");
        for (String table : TARGETS) {
            System.out.printf("BEGIN_MAPS\t%s%n", table);
            SlotMapProbe.main(new String[] {path.toString(), table, "1"});
            System.out.printf("END_MAPS\t%s%n", table);
        }
    }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length == 3 && arguments[0].equals("generate")) {
            generate(Path.of(arguments[1]), arguments[2]);
        } else if (arguments.length == 2 && arguments[0].equals("describe")) {
            describe(Path.of(arguments[1]));
        } else {
            throw new IllegalArgumentException("Expected generate FRESH_PATH PROFILE or describe PATH");
        }
    }
}
