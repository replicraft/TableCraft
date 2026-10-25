import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import java.nio.file.Files;
import java.nio.file.Path;

/** Original MIT recipe: inert links authored only through public Jackcess APIs. */
public class LinkedTableFixture {
    public static void main(String[] args) throws Exception {
        if (args.length != 2) {
            throw new IllegalArgumentException("Expected fresh directory and file format");
        }
        Path directory = Path.of(args[0]);
        Database.FileFormat format = Database.FileFormat.valueOf(args[1]);
        String suffix = format == Database.FileFormat.V2000 ? ".mdb" : ".accdb";
        Files.createDirectories(directory);
        Path frontend = directory.resolve("frontend" + suffix);
        Path backend = directory.resolve("backend" + suffix);
        if (Files.exists(frontend) || Files.exists(backend)) {
            throw new IllegalArgumentException("Outputs must not exist");
        }

        try (Database db = new DatabaseBuilder(backend).setFileFormat(format).create()) {
            db.setEvaluateExpressions(false);
            Table table = new TableBuilder("Remote table")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("Label", DataType.TEXT).setLengthInUnits(100))
                .toTable(db);
            table.addRow(1, "one é中");
            table.addRow(2, "two");
        }
        try (Database db = new DatabaseBuilder(frontend).setFileFormat(format).create()) {
            db.setEvaluateExpressions(false);
            db.createLinkedTable("Backend link", backend.getFileName().toString(), "Remote table");
            db.createLinkedTable("Unicode é中", "relative/é中.accdb", "Remote é中");
            db.createLinkedTable("Opaque identifiers", " a;token=opaque.accdb ", "SELECT 1");
            String longDatabase = "missing/" + "é中".repeat(2000) + ".accdb";
            db.createLinkedTable("Long database", longDatabase, "Remote table");
            db.flush();
        }
    }
}
