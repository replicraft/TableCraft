import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Table;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.Arrays;

// Original repository reference: explicit inert SDK mutation on a fresh copy.
// Requires the previously reviewed Jackcess 5.0.4 JAR; no database is supplied.
public class CreatedTableColumnOracle {
    public static void main(String[] args) throws Exception {
        if (args.length < 4 || args.length > 5) {
            throw new IllegalArgumentException(
                "Expected original, fresh after path, table, column, optional --default-zero"
            );
        }
        boolean defaultZero = args.length == 5;
        if (defaultZero && !args[4].equals("--default-zero")) {
            throw new IllegalArgumentException("Unknown target-property option");
        }
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
            ColumnBuilder column = new ColumnBuilder(args[3], DataType.LONG);
            if (defaultZero) {
                column.putProperty("DefaultValue", DataType.MEMO, "0");
            }
            column.addToTable(table);
        }
        try (Database database = new DatabaseBuilder(after).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            Table table = database.getTable(args[2]);
            for (Column column : table.getColumns()) {
                System.out.println(column.getName() + "\t" + column.getType()
                    + "\t" + column.isAutoNumber() + "\t"
                    + column.getProperties().getValue("DefaultValue"));
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
