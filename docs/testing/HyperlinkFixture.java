import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import java.nio.file.Files;
import java.nio.file.Path;

// Original repository-licensed inert Hyperlink-field fixture, not Access execution.
public class HyperlinkFixture {
    public static void main(String[] args) throws Exception {
        if (args.length != 2) {
            throw new IllegalArgumentException("Expected fresh path and file-format name");
        }

        Path path = Path.of(args[0]);
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Output must not exist");
        }
        Database.FileFormat format = Database.FileFormat.valueOf(args[1]);
        try (Database db = new DatabaseBuilder(path).setFileFormat(format).create()) {
            Table table = new TableBuilder("Hyperlinks")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("Link", DataType.MEMO).setHyperlink(true))
                .toTable(db);

            table.addRow(1, "Microsoft#https://www.microsoft.com#");
            table.addRow(2, "Customers#https://www.microsoft.com#Form Customers");
            table.addRow(3, "##Form Customers#Enter Information");
            table.addRow(4, "#https://example.invalid/#");
            table.addRow(5, "名🦀#résumé#中#tip#tail");
            table.addRow(6, null);
            table.addRow(7, "");
            table.addRow(8, "label");
        }
    }
}
