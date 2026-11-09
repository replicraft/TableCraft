import com.healthmarketscience.jackcess.Cursor;
import com.healthmarketscience.jackcess.CursorBuilder;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.impl.RowIdImpl;
import com.healthmarketscience.jackcess.impl.TableImpl;
import java.nio.file.Path;

// Original inert location report. It never edits storage or evaluates expressions.
public class WideRowLocationOracle {
    public static void main(String[] args) throws Exception {
        if (args.length != 2) {
            throw new IllegalArgumentException("Expected database and table");
        }
        try (Database database = new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            Table table = database.getTable(args[1]);
            Cursor cursor = CursorBuilder.createCursor(table);
            Row row;
            while ((row = cursor.getNextRow()) != null) {
                RowIdImpl location = (RowIdImpl) row.getId();
                System.out.println(row.getInt("ID") + "\t" + location.getPageNumber()
                    + "\t" + location.getRowNumber() + "\t" + ((TableImpl) table).getTableDefPageNumber());
            }
        }
    }
}
