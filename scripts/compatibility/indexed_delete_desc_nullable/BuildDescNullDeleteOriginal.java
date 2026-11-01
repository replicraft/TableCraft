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
public final class BuildDescNullDeleteOriginal {
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
                .addIndex(new IndexBuilder("GroupOrder").addColumns(false, "Guard"))
                .addIndex(new IndexBuilder("GroupAlias").addColumns(false, "Guard"))
                .toTable(database);

            items.addRow(1, -1, null);
            items.addRow(2, 0, 1);
            items.addRow(3, 1, 2);
            database.flush();
            if (Files.size(target) > FILE_CEILING) {
                throw new AssertionError("Original exceeds its source envelope");
            }
            System.out.printf("CREATED\t%s\titems3\tguard-null1\tdesc-aliases2%n", args[1]);
        }
    }
}
