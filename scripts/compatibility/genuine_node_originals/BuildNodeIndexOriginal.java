import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.IndexBuilder;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import java.nio.channels.FileChannel;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.Set;

/** Public SDK original creation only; no TableCraft writer or AFTER model. */
public final class BuildNodeIndexOriginal {
    private static final int ROWS = 800;
    private static final long FILE_CEILING = 8L * 1024 * 1024;
    private static final Set<String> PROFILES = Set.of("V2016", "V2019");

    public static void main(String[] args) throws Exception {
        if (args.length != 2 || !PROFILES.contains(args[1])) {
            throw new IllegalArgumentException("Expected fresh PATH V2016|V2019");
        }
        Path target = Path.of(args[0]);
        if (Files.exists(target)) {
            throw new IllegalArgumentException("Original source path must be absent");
        }

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
                throw new AssertionError("No external database is authorized");
            });

            Table items = new TableBuilder("Items")
                .addColumn(new ColumnBuilder("Id", DataType.LONG).setAutoNumber(false))
                .addColumn(new ColumnBuilder("Score", DataType.LONG).setAutoNumber(false))
                .addColumn(new ColumnBuilder("Guard", DataType.LONG).setAutoNumber(false))
                .addIndex(new IndexBuilder("PrimaryID").addColumns("Id").setPrimaryKey())
                .addIndex(new IndexBuilder("PrimaryAlias").addColumns("Id")
                    .setRequired().setUnique())
                .addIndex(new IndexBuilder("GroupOrder").addColumns("Guard"))
                .addIndex(new IndexBuilder("GroupAlias").addColumns("Guard"))
                .toTable(database);

            for (int position = 0; position < ROWS; position++) {
                items.addRow(position + 1, position - 400, position % 7);
            }

            Table sentinel = new TableBuilder("Sentinel")
                .addColumn(new ColumnBuilder("Marker", DataType.LONG).setAutoNumber(false))
                .toTable(database);
            sentinel.addRow(999);
            database.flush();
            if (Files.size(target) > FILE_CEILING) {
                throw new AssertionError("Original exceeds its source envelope");
            }
            System.out.printf("CREATED\t%s\titems800\tsentinel1%n", args[1]);
        }
    }
}
