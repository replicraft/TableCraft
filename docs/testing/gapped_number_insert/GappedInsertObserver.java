import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.Cursor;
import com.healthmarketscience.jackcess.CursorBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Index;
import com.healthmarketscience.jackcess.PropertyMap;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import com.healthmarketscience.jackcess.impl.TableImpl;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;

// First-party inert reader of a frozen authored row hypothesis; no writer calls.
public final class GappedInsertObserver {
    public static void main(String[] args) throws Exception {
        if (args.length != 2) {
            throw new IllegalArgumentException("Expected copied database and row count");
        }
        Path path = Path.of(args[0]);
        int expectedRows = Integer.parseInt(args[1]);
        byte[] before = Files.readAllBytes(path);
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            for (String name : new String[] {"Track Name AutoCorrect Info", "Perform Name AutoCorrect"}) {
                PropertyMap.Property property = database.getDatabaseProperties().get(name);
                if (property == null || property.getType() != DataType.LONG || !property.isDdl()
                    || !(property.getValue() instanceof Number)
                    || ((Number) property.getValue()).longValue() != 0) {
                    throw new IllegalStateException("Expected actual stored LONG/DDL OFF");
                }
                System.out.println("OPTION\t" + name + "\t" + property.getValue());
            }
            Table table = database.getTable("Table1");
            if (table.getRowCount() != expectedRows || table.getColumnCount() != 2
                || ((TableImpl) table).getMaxColumnCount() != 3 || table.getIndexes().size() != 1) {
                throw new IllegalStateException("Unexpected gapped table geometry");
            }
            String[] names = {"ID", "Score2"};
            int[] numbers = {0, 2};
            int[] offsets = {0, 8};
            for (int position = 0; position < names.length; position++) {
                Column column = table.getColumns().get(position);
                ColumnImpl nativeColumn = (ColumnImpl) column;
                if (!column.getName().equals(names[position]) || column.getType() != DataType.LONG
                    || nativeColumn.getColumnNumber() != numbers[position]
                    || column.getColumnId() != numbers[position]
                    || nativeColumn.getFixedDataOffset() != offsets[position]) {
                    throw new IllegalStateException("Unexpected true field identity or offset");
                }
                System.out.println("COLUMN\t" + column.getName() + "\t"
                    + nativeColumn.getColumnNumber() + "\t" + column.getColumnId() + "\t"
                    + nativeColumn.getFixedDataOffset());
                for (PropertyMap.Property property : column.getProperties()) {
                    System.out.println("PROPERTY\t" + column.getName() + "\t" + property.getName()
                        + "\t" + property.getType() + "\t" + property.isDdl()
                        + "\t" + property.getValue());
                }
            }
            Index index = table.getIndexes().get(0);
            if (!index.isPrimaryKey() || !index.isUnique() || index.isForeignKey()
                || index.getColumnCount() != 1
                || !index.getColumns().get(0).getColumn().getName().equals("ID")) {
                throw new IllegalStateException("Expected original ID primary key");
            }
            int sequential = read(CursorBuilder.createCursor(table), expectedRows);
            int indexed = read(CursorBuilder.createCursor(index), expectedRows);
            System.out.println("ROWS\t" + sequential + "\t" + indexed);
        }
        if (!Arrays.equals(before, Files.readAllBytes(path))) {
            throw new IllegalStateException("Read-only observation changed copied bytes");
        }
    }

    private static int read(Cursor cursor, int expectedRows) throws Exception {
        int count = 0;
        Row row;
        while ((row = cursor.getNextRow()) != null) {
            count++;
            if (expectedRows != 1 || count != 1 || row.size() != 2
                || !Integer.valueOf(1).equals(row.get("ID"))
                || !Integer.valueOf(0).equals(row.get("Score2"))) {
                throw new IllegalStateException("Unexpected native row values");
            }
        }
        if (count != expectedRows) {
            throw new IllegalStateException("Sequential/indexed membership differs");
        }
        return count;
    }
}
