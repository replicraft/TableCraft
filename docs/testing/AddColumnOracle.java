import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Table;
import java.nio.file.Files;
import java.nio.file.Path;

// Original inert public-SDK operation on a new copy; no production layout code.
public class AddColumnOracle {
    public static void main(String[] args) throws Exception {
        if (args.length != 2) {
            throw new IllegalArgumentException("Expected original and fresh destination");
        }
        Path before = Path.of(args[0]);
        Path after = Path.of(args[1]);
        Files.copy(before, after);
        try (Database database = new DatabaseBuilder(after).open()) {
            database.setEvaluateExpressions(false);
            Table table = database.getTable("Items");
            if (table.getRowCount() != 0 || table.getColumnCount() != 2 || !table.getIndexes().isEmpty()) {
                throw new IllegalArgumentException("Expected empty two-field unindexed source");
            }
            new ColumnBuilder("Extra", DataType.LONG).addToTable(table);
            for (Column column : table.getColumns()) {
                System.out.println(column.getName() + "\t" + column.getType() + "\t" + column.getColumnIndex());
            }
            System.out.println("Rows\t" + table.getRowCount());
            System.out.println("Created\t" + table.getCreatedDate());
            System.out.println("Updated\t" + table.getUpdatedDate());
        }
    }
}
