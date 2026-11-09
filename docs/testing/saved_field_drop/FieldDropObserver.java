import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Index;
import com.healthmarketscience.jackcess.PropertyMap;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import com.healthmarketscience.jackcess.impl.PropertyMapImpl;
import com.healthmarketscience.jackcess.impl.TableImpl;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;

// First-party inert observation of generated raw hypotheses; no mutation calls.
public final class FieldDropObserver {
    public static void main(String[] args) throws Exception {
        if (args.length != 1) {
            throw new IllegalArgumentException("Expected one generated fixture");
        }
        Path path = Path.of(args[0]);
        byte[] before = Files.readAllBytes(path);
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            for (String name : new String[] {"Track Name AutoCorrect Info", "Perform Name AutoCorrect"}) {
                PropertyMap.Property property = database.getDatabaseProperties().get(name);
                if (property == null || property.getType() != DataType.LONG || !property.isDdl()
                    || !(property.getValue() instanceof Number)
                    || ((Number) property.getValue()).longValue() != 0) {
                    throw new IllegalStateException("Expected stored explicit OFF");
                }
                System.out.println("OPTION\t" + name + "\t" + property.getValue());
            }
            Table table = database.getTable("Table1");
            if (table.getRowCount() != 0 || table.getIndexes().size() != 1) {
                throw new IllegalStateException("Expected empty retained-primary source");
            }
            TableImpl nativeTable = (TableImpl) table;
            System.out.println("TABLE\t" + table.getColumnCount() + "\t"
                + nativeTable.getMaxColumnCount() + "\t" + table.getRowCount());
            for (Column column : table.getColumns()) {
                ColumnImpl nativeColumn = (ColumnImpl) column;
                System.out.println("COLUMN\t" + nativeColumn.getColumnNumber() + "\t"
                    + column.getColumnId() + "\t" + column.getName() + "\t" + column.getType()
                    + "\t" + column.getLengthInUnits() + "\t" + column.isCompressedUnicode()
                    + "\t" + nativeColumn.getVarLenTableIndex() + "\t"
                    + nativeColumn.getFixedDataOffset());
                for (PropertyMap.Property property : column.getProperties()) {
                    System.out.println("PROPERTY\t" + column.getName() + "\t" + property.getName()
                        + "\t" + property.getType() + "\t" + property.isDdl()
                        + "\t" + property.getValue());
                }
            }
            for (PropertyMapImpl group : nativeTable.getPropertyMaps()) {
                System.out.println("GROUP\t" + group.getType() + "\t" + group.getName()
                    + "\t" + group.getSize());
            }
            for (Index index : table.getIndexes()) {
                System.out.println("INDEX\t" + index.getName() + "\t" + index.isPrimaryKey()
                    + "\t" + index.isUnique() + "\t" + index.isForeignKey());
                if (!index.isPrimaryKey() || index.getColumnCount() != 1
                    || !index.getColumns().get(0).getColumn().getName().equals("ID")) {
                    throw new IllegalStateException("Expected retained ID-only primary index");
                }
                for (Index.Column component : index.getColumns()) {
                    ColumnImpl column = (ColumnImpl) component.getColumn();
                    System.out.println("KEY\t" + column.getName() + "\t" + column.getColumnNumber());
                }
            }
        }
        if (!Arrays.equals(before, Files.readAllBytes(path))) {
            throw new IllegalStateException("Read-only observer changed source bytes");
        }
    }
}
