import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.PropertyMap;
import com.healthmarketscience.jackcess.Table;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;

// Original metadata-only reader for generated raw rename hypotheses. No writes.
public final class FieldRenameObserver {
    public static void main(String[] args) throws Exception {
        if (args.length != 1) {
            throw new IllegalArgumentException("Expected generated fixture path");
        }
        Path path = Path.of(args[0]);
        byte[] before = Files.readAllBytes(path);
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            PropertyMap options = database.getDatabaseProperties();
            for (String name : new String[] {"Track Name AutoCorrect Info", "Perform Name AutoCorrect"}) {
                PropertyMap.Property property = options.get(name);
                if (property == null || property.getType() != DataType.LONG || !property.isDdl()
                    || !(property.getValue() instanceof Number)
                    || ((Number) property.getValue()).longValue() != 0) {
                    throw new IllegalStateException("Expected actual explicit OFF settings");
                }
                System.out.println("OPTION\t" + name + "\t" + property.getType()
                    + "\t" + property.isDdl() + "\t" + property.getValue());
            }
            Table table = database.getTable("Table1");
            if (table.getRowCount() != 0 || table.getIndexes().size() != 1) {
                throw new IllegalStateException("Expected empty generated-primary fixture");
            }
            for (Column column : table.getColumns()) {
                System.out.println("COLUMN\t" + column.getColumnIndex() + "\t" + column.getColumnId()
                    + "\t" + column.getName() + "\t" + column.getType()
                    + "\t" + column.getLengthInUnits() + "\t" + column.isCompressedUnicode());
                for (PropertyMap.Property property : column.getProperties()) {
                    System.out.println("PROPERTY\t" + column.getColumnIndex() + "\t"
                        + property.getName() + "\t" + property.getType() + "\t"
                        + property.isDdl() + "\t" + property.getValue());
                }
            }
            System.out.println("TABLE\t" + table.getName() + "\t" + table.getColumnCount()
                + "\t" + table.getRowCount() + "\t" + table.getIndexes().size());
        }
        if (!Arrays.equals(before, Files.readAllBytes(path))) {
            throw new IllegalStateException("Readonly observation changed fixture bytes");
        }
    }
}
