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
import java.util.Objects;

// First-party inert read of frozen dense populated sources; no writer calls.
public final class PopulatedRenameObserver {
    public static void main(String[] args) throws Exception {
        if (args.length != 3) {
            throw new IllegalArgumentException("Expected copied fixture, field name and comma-separated values");
        }
        Path path = Path.of(args[0]);
        String field = args[1];
        String[] tokens = args[2].split(",", -1);
        if (tokens.length < 1 || tokens.length > 255 || Files.size(path) != 45056) {
            throw new IllegalArgumentException("Outside the bounded authored row family");
        }
        Integer[] values = new Integer[tokens.length];
        for (int index = 0; index < values.length; index++) {
            values[index] = tokens[index].equals("NULL") ? null : Integer.valueOf(tokens[index]);
        }
        byte[] before = Files.readAllBytes(path);
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            database.setLinkResolver((source, token) -> {
                throw new AssertionError("Linked targets are not authorized");
            });
            for (String name : new String[] {"Track Name AutoCorrect Info", "Perform Name AutoCorrect"}) {
                PropertyMap.Property property = database.getDatabaseProperties().get(name);
                if (property == null || property.getType() != DataType.LONG || !property.isDdl()
                    || !Integer.valueOf(0).equals(property.getValue())) {
                    throw new IllegalStateException("Expected stored LONG/DDL explicit OFF");
                }
                System.out.println("OPTION\t" + name + "\t" + property.getType()
                    + "\t" + property.isDdl() + "\t" + property.getValue());
            }
            Table table = database.getTable("Table1");
            if (table.getRowCount() != values.length || table.getColumnCount() != 2
                || ((TableImpl) table).getMaxColumnCount() != 2 || table.getIndexes().size() != 1) {
                throw new IllegalStateException("Unexpected dense source geometry");
            }
            String[] names = {"ID", field};
            for (int position = 0; position < names.length; position++) {
                Column column = table.getColumns().get(position);
                ColumnImpl nativeColumn = (ColumnImpl) column;
                if (!column.getName().equals(names[position]) || column.getType() != DataType.LONG
                    || nativeColumn.getColumnNumber() != position || column.getColumnId() != position
                    || nativeColumn.getFixedDataOffset() != position * 4
                    || column.isAutoNumber() != (position == 0)) {
                    throw new IllegalStateException("Field identity, storage or generation changed");
                }
                System.out.println("COLUMN\t" + column.getColumnIndex() + "\t" + column.getColumnId()
                    + "\t" + column.getName() + "\t" + column.getType()
                    + "\t" + column.getLengthInUnits() + "\t" + column.isCompressedUnicode());
                System.out.println("NATIVE\t" + position + "\t" + nativeColumn.getColumnNumber()
                    + "\t" + nativeColumn.getFixedDataOffset() + "\t" + column.isAutoNumber());
                for (PropertyMap.Property property : column.getProperties()) {
                    System.out.println("PROPERTY\t" + position + "\t" + property.getName()
                        + "\t" + property.getType() + "\t" + property.isDdl() + "\t" + property.getValue());
                }
            }
            Index primary = table.getIndexes().get(0);
            if (!primary.isPrimaryKey() || !primary.isUnique() || primary.isForeignKey()
                || primary.getColumnCount() != 1
                || !primary.getColumns().get(0).getColumn().getName().equals("ID")) {
                throw new IllegalStateException("Expected original ID-only primary index");
            }
            read(CursorBuilder.createCursor(table), field, values, "sequential");
            read(CursorBuilder.createCursor(primary), field, values, "indexed");
            System.out.println("TABLE\t" + table.getName() + "\t" + table.getColumnCount()
                + "\t" + table.getRowCount() + "\t" + table.getIndexes().size());
        }
        if (!Arrays.equals(before, Files.readAllBytes(path))) {
            throw new IllegalStateException("Read-only observation changed copied bytes");
        }
    }

    private static void read(Cursor cursor, String field, Integer[] values, String mode) throws Exception {
        int count = 0;
        Row row;
        while ((row = cursor.getNextRow()) != null) {
            if (count >= values.length || row.size() != 2
                || !Integer.valueOf(count + 1).equals(row.get("ID"))
                || !Objects.equals(values[count], row.get(field))) {
                throw new IllegalStateException("Unexpected actual row identity/value in " + mode);
            }
            System.out.println("ROW\t" + mode + "\t" + (count + 1) + "\t"
                + (row.get(field) == null ? "NULL" : row.get(field)));
            count++;
        }
        if (count != values.length) {
            throw new IllegalStateException("Actual cursor membership differs in " + mode);
        }
    }
}
