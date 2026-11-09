import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.Arrays;

// Original inert SDK recipe. The explicit compression switch separates the
// current TableCraft creator from Microsoft's documented Access UI default.
public class CreatedShortTextColumnOracle {
    public static void main(String[] args) throws Exception {
        if (args.length != 6) {
            throw new IllegalArgumentException(
                "Expected original, fresh after, table, column, units, compressed Boolean"
            );
        }
        int units = Integer.parseInt(args[4]);
        if (units < 1 || units > 255
            || !(args[5].equals("true") || args[5].equals("false"))) {
            throw new IllegalArgumentException("Unsupported explicit text declaration");
        }
        boolean compressed = Boolean.parseBoolean(args[5]);
        Path original = Path.of(args[0]);
        Path after = Path.of(args[1]);
        byte[] originalBytes = Files.readAllBytes(original);
        Files.copy(original, after);
        try (Database database = new DatabaseBuilder(after).open()) {
            database.setEvaluateExpressions(false);
            Table table = database.getTable(args[2]);
            if (table.getRowCount() != 0 || table.getIndexes().size() != 1
                || !table.getColumns().get(0).isAutoNumber()) {
                throw new IllegalArgumentException("Expected empty generated-primary source");
            }
            new ColumnBuilder(args[3], DataType.TEXT)
                .setLengthInUnits(units)
                .setCompressedUnicode(compressed)
                .putProperty("Required", DataType.BOOLEAN, false)
                .putProperty("AllowZeroLength", DataType.BOOLEAN, true)
                .addToTable(table);
        }
        try (Database database = new DatabaseBuilder(after).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            Table table = database.getTable(args[2]);
            for (Column column : table.getColumns()) {
                ColumnImpl nativeColumn = (ColumnImpl)column;
                System.out.println("Column\t" + column.getName() + "\t" + column.getType()
                    + "\tunits=" + column.getLengthInUnits()
                    + "\tnumber=" + nativeColumn.getColumnNumber()
                    + "\tvariable=" + nativeColumn.getVarLenTableIndex()
                    + "\tfixedOffset=" + nativeColumn.getFixedDataOffset()
                    + "\tcompressed=" + column.isCompressedUnicode()
                    + "\trequired=" + column.getProperties().getValue("Required")
                    + "\tallowZeroLength=" + column.getProperties().getValue("AllowZeroLength")
                    + "\tdefault=" + column.getProperties().getValue("DefaultValue")
                    + "\tsort=" + nativeColumn.getTextSortOrder());
            }
            System.out.println("Rows\t" + table.getRowCount());
            System.out.println("Indexes\t" + table.getIndexes().size());
            System.out.println("Created\t" + table.getCreatedDate());
            System.out.println("Updated\t" + table.getUpdatedDate());
        }
        if (!Arrays.equals(originalBytes, Files.readAllBytes(original))) {
            throw new IllegalStateException("Original source changed");
        }
        System.out.println("BeforeSHA256\t" + hash(originalBytes));
        System.out.println("AfterSHA256\t" + hash(Files.readAllBytes(after)));
    }

    private static String hash(byte[] bytes) throws Exception {
        byte[] digest = MessageDigest.getInstance("SHA-256").digest(bytes);
        StringBuilder result = new StringBuilder();
        for (byte value : digest) {
            result.append(String.format("%02x", value & 255));
        }
        return result.toString();
    }
}
