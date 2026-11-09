import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.PropertyMap;
import com.healthmarketscience.jackcess.Table;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;

// Original metadata-only follow-up: distinguish missing DefaultValue from null.
public class ShortTextProperties {
    public static void main(String[] args) throws Exception {
        if (args.length != 4) {
            throw new IllegalArgumentException("Expected file, table, column, compressed Boolean");
        }
        Path path = Path.of(args[0]);
        byte[] before = Files.readAllBytes(path);
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            Table table = database.getTable(args[1]);
            Column column = table.getColumn(args[2]);
            PropertyMap properties = column.getProperties();
            if (column.getType() != DataType.TEXT || column.getLengthInUnits() != 255
                || column.isCompressedUnicode() != Boolean.parseBoolean(args[3])
                || table.getRowCount() != 0 || table.getIndexes().size() != 1
                || properties.get("Required") == null
                || properties.get("Required").getType() != DataType.BOOLEAN
                || !Boolean.FALSE.equals(properties.getValue("Required"))
                || properties.get("AllowZeroLength") == null
                || properties.get("AllowZeroLength").getType() != DataType.BOOLEAN
                || !Boolean.TRUE.equals(properties.getValue("AllowZeroLength"))
                || properties.get("DefaultValue") != null) {
                throw new IllegalStateException("Exact Short Text declaration differs");
            }
            for (PropertyMap.Property property : properties) {
                System.out.println(property.getName() + "\t" + property.getType()
                    + "\t" + property.getValue());
            }
            System.out.println("DefaultValuePresent\tfalse");
        }
        if (!Arrays.equals(before, Files.readAllBytes(path))) {
            throw new IllegalStateException("Read-only metadata changed file bytes");
        }
    }
}
