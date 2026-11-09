import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HexFormat;

/** Original inert cached Memo corpus and independent read-only Text oracle. */
public class CalculatedMemoFixture {
    public static void main(String[] args) throws Exception {
        if (args.length == 3 && args[0].equals("--decode")) {
            try (Database database = new DatabaseBuilder(Path.of(args[1])).setReadOnly(true).open()) {
                database.setEvaluateExpressions(false);
                ColumnImpl column = (ColumnImpl) database.getTable("CalculatedMemos").getColumn("Wide");
                String value = (String) column.read(HexFormat.of().parseHex(args[2]), ByteOrder.LITTLE_ENDIAN);
                System.out.println("T" + HexFormat.of().formatHex(value.getBytes(StandardCharsets.UTF_8)));
            }
            return;
        }
        if (args.length != 2) {
            throw new IllegalArgumentException("Expected fresh path and V2016/V2019, or --inspect path");
        }
        if (args[0].equals("--inspect")) {
            inspect(Path.of(args[1]));
            return;
        }
        Path path = Path.of(args[0]);
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Output must not exist");
        }
        Database.FileFormat format = switch (args[1]) {
            case "V2016" -> Database.FileFormat.V2016;
            case "V2019" -> Database.FileFormat.V2019;
            default -> throw new IllegalArgumentException("Unsupported format");
        };
        try (Database database = new DatabaseBuilder(path).setFileFormat(format).create()) {
            database.setEvaluateExpressions(false);
            Table table = builder("CalculatedMemos").toTable(database);
            Object[] values = {"ASCII", "", null, "é🦀\t\"\\\n", "é🦀".repeat(3000), "ASCII".repeat(1000), "late-cache"};
            int identity = 1;
            for (Object value : values) {
                table.addRow(identity++, value, value);
            }
            builder("EmptyCalculatedMemos").toTable(database);
        }
    }

    private static TableBuilder builder(String name) {
        return new TableBuilder(name)
            .addColumn(new ColumnBuilder("ID", DataType.LONG))
            .addColumn(new ColumnBuilder("Wide", DataType.MEMO).setCalculatedInfo("\"prefix\" & [ID]"))
            .addColumn(new ColumnBuilder("Compact", DataType.MEMO).setCalculatedInfo("\"prefix\" & [ID]").setCompressedUnicode(true));
    }

    private static void inspect(Path path) throws Exception {
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            Table table = database.getTable("CalculatedMemos");
            for (String name : new String[] {"Wide", "Compact"}) {
                var column = table.getColumn(name);
                System.out.printf("D\t%s\t%s\t%d\t%s%n", name, column.getType(), column.getLength(), column.isCompressedUnicode());
            }
            int ordinal = 0;
            for (Row row : table) {
                for (String name : new String[] {"Wide", "Compact"}) {
                    Object value = row.get(name);
                    if (value != null && !(value instanceof String)) {
                        throw new IllegalStateException("Expected stored Text getter");
                    }
                    String encoded = value == null ? "N" : "T" + HexFormat.of().formatHex(((String) value).getBytes(StandardCharsets.UTF_8));
                    System.out.printf("R\t%d\t%s\t%s%n", ordinal, name, encoded);
                }
                ordinal++;
            }
        }
    }
}
