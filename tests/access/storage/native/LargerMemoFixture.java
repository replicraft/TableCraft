import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.IndexBuilder;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.HexFormat;
import java.util.Set;

/** Original inert Memo data and read-only getter oracle; no SQL or VBA executes. */
public class LargerMemoFixture {
    public static void main(String[] args) throws Exception {
        if (args.length != 2) {
            throw new IllegalArgumentException("Expected fresh path/profile or --inspect/path");
        }
        if (args[0].equals("--inspect")) {
            try (Database database = new DatabaseBuilder(Path.of(args[1])).setReadOnly(true).open()) {
                database.setEvaluateExpressions(false);
                for (var row : database.getTable("LargeMemos")) {
                    String value = row.getString("Memo");
                    byte[] data = value == null ? null : value.getBytes(StandardCharsets.UTF_8);
                    String hash = data == null ? "NULL" : HexFormat.of().formatHex(
                        MessageDigest.getInstance("SHA-256").digest(data));
                    System.out.printf("%d\t%d\t%d\t%s%n", row.getInt("ID"),
                        value == null ? -1 : value.length(), data == null ? -1 : data.length, hash);
                }
            }
            return;
        }

        Path path = Path.of(args[0]);
        if (Files.exists(path) || !Set.of("V2000", "V2007", "V2016", "V2019").contains(args[1])) {
            throw new IllegalArgumentException("Expected fresh path and qualified profile");
        }
        try (Database database = new DatabaseBuilder(path)
                .setFileFormat(Database.FileFormat.valueOf(args[1])).setAutoSync(false).create()) {
            database.setEvaluateExpressions(false);
            Table table = new TableBuilder("LargeMemos")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("Memo", DataType.MEMO))
                .addIndex(new IndexBuilder("PrimaryKey").addColumns("ID").setPrimaryKey())
                .toTable(database);
            table.addRow(1, "a".repeat(3_145_728));
            table.addRow(2, "中".repeat(600_000));
            table.addRow(3, "é🦀".repeat(300_000));
            table.addRow(4, "");
            table.addRow(5, null);
            database.flush();
        }
    }
}
