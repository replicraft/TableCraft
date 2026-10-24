import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.HexFormat;
import java.util.Set;

/** Original inert OLE bytes; no application object or expression is executed. */
public class LargerEncodedValueFixture {
    public static void main(String[] args) throws Exception {
        if (args.length != 2) {
            throw new IllegalArgumentException("Expected fresh path/profile or --inspect/path");
        }
        if (args[0].equals("--inspect")) {
            try (Database database = new DatabaseBuilder(Path.of(args[1])).setReadOnly(true).open()) {
                database.setEvaluateExpressions(false);
                for (var row : database.getTable("EncodedValues")) {
                    byte[] data = row.getBytes("Payload");
                    String hash = data == null ? "NULL" : HexFormat.of().formatHex(
                        MessageDigest.getInstance("SHA-256").digest(data));
                    System.out.printf("%d\t%d\t%s%n", row.getInt("ID"),
                        data == null ? -1 : data.length, hash);
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
            Table table = new TableBuilder("EncodedValues")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("Payload", DataType.OLE)).toTable(database);
            for (int id = 1; id <= 2; id++) {
                byte[] data = new byte[id == 1 ? 4_194_304 : 20_971_520];
                for (int index = 0; index < data.length; index++) {
                    data[index] = (byte) (index * 17 + index / 256);
                }
                table.addRow(id, data);
            }
            table.addRow(3, new byte[0]);
            table.addRow(4, null);
            database.flush();
        }
    }
}
