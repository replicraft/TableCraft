import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.IndexBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import java.math.BigDecimal;
import java.nio.channels.FileChannel;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.Set;

/** SDK ORIGINALS only; no omitted-default request or TableCraft producer runs. */
public final class BuildDefaultOriginal {
    private static final Set<String> FORMATS = Set.of("V2016", "V2019");
    private static final long FILE_CEILING = 8L * 1024 * 1024;

    private static void require(boolean condition, String message) {
        if (!condition) {
            throw new IllegalStateException(message);
        }
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 2 || !FORMATS.contains(args[1])) {
            throw new IllegalArgumentException("Expected absent PATH V2016|V2019");
        }
        Path target = Path.of(args[0]);
        require(!Files.exists(target), "Original target must be absent");
        try (FileChannel raw = FileChannel.open(target, StandardOpenOption.CREATE_NEW,
                StandardOpenOption.READ, StandardOpenOption.WRITE);
             BoundedFileChannel bounded = new BoundedFileChannel(raw, FILE_CEILING);
             Database database = new DatabaseBuilder(target.toFile())
                .setChannel(bounded)
                .setFileFormat(Database.FileFormat.valueOf(args[1]))
                .create()) {
            database.setEvaluateExpressions(false);
            database.setEnforceForeignKeys(true);
            database.setLinkResolver((source, token) -> {
                throw new AssertionError("External targets are outside this original recipe");
            });
            Table defaults = new TableBuilder("Defaults")
                .addColumn(new ColumnBuilder("Id", DataType.LONG).setAutoNumber(false))
                .addColumn(new ColumnBuilder("Score", DataType.LONG)
                    .putProperty("DefaultValue", "7"))
                .addColumn(new ColumnBuilder("Caption", DataType.TEXT)
                    .setLengthInUnits(40).setCompressedUnicode(false)
                    .putProperty("DefaultValue", "\"seed\""))
                .addColumn(new ColumnBuilder("Flag", DataType.BOOLEAN)
                    .putProperty("DefaultValue", "True"))
                .addColumn(new ColumnBuilder("Money", DataType.MONEY)
                    .putProperty("DefaultValue", "-1.0001"))
                .addColumn(new ColumnBuilder("Optional", DataType.LONG))
                .addIndex(new IndexBuilder("PrimaryID").addColumns("Id").setPrimaryKey())
                .addIndex(new IndexBuilder("PrimaryAlias").addColumns("Id")
                    .setRequired().setUnique())
                .toTable(database);
            // Every default-bearing field is explicitly supplied. The sole
            // Java null belongs to an ordinary field with no default at all.
            defaults.addRow(1, -1, "original", false, new BigDecimal("2.5000"), null);

            Table numbered = new TableBuilder("Numbered")
                .addColumn(new ColumnBuilder("Earlier", DataType.LONG))
                .addColumn(new ColumnBuilder("Id", DataType.LONG).setAutoNumber(true))
                .addColumn(new ColumnBuilder("Score", DataType.LONG))
                .addIndex(new IndexBuilder("PrimaryID").addColumns("Id").setPrimaryKey())
                .addIndex(new IndexBuilder("PrimaryAlias").addColumns("Id")
                    .setRequired().setUnique())
                .toTable(database);
            for (int id = 1; id <= 3; id++) {
                Object[] added = numbered.addRow(100 + id, Column.AUTO_NUMBER, 10 * id);
                require(added[1].equals(id), "SDK construction sequence must be observed1..3");
            }
            Row deleted = null;
            for (Row row : numbered) {
                if (Integer.valueOf(3).equals(row.get("Id"))) {
                    deleted = row;
                }
            }
            require(deleted != null, "Actual highest original row must exist");
            numbered.deleteRow(deleted);
            require(numbered.getRowCount() == 2, "Deleted-high original live count");

            Table nonPrimary = new TableBuilder("NonPrimary")
                .addColumn(new ColumnBuilder("Id", DataType.LONG).setAutoNumber(false))
                .addColumn(new ColumnBuilder("Ticket", DataType.LONG).setAutoNumber(true))
                .addIndex(new IndexBuilder("PrimaryID").addColumns("Id").setPrimaryKey())
                .addIndex(new IndexBuilder("TicketUnique").addColumns("Ticket").setUnique())
                .toTable(database);
            Object[] nonPrimaryRow = nonPrimary.addRow(50, Column.AUTO_NUMBER);
            require(nonPrimaryRow[1].equals(1), "Observe the nonprimary SDK-generated original Ticket");

            Table sentinel = new TableBuilder("Sentinel")
                .addColumn(new ColumnBuilder("Marker", DataType.LONG))
                .toTable(database);
            sentinel.addRow(999);
            database.flush();
            require(Files.size(target) <= FILE_CEILING, "Original exceeds the bounded envelope");
            require(!database.isEvaluateExpressions(), "Default/property expressions must remain inert");
            System.out.printf("CREATED\t%s\tDefaults1\tNumbered2\tNonPrimary1\tSentinel1%n", args[1]);
        }
    }
}
