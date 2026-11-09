import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import com.healthmarketscience.jackcess.impl.RowIdImpl;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HexFormat;

/** Original seed producer and inert stored-text/encoded-stream getter oracle. */
public class CompressedWideFixture {
    private static String hex(String value) {
        return HexFormat.of().formatHex(value.getBytes(StandardCharsets.UTF_8));
    }

    public static void main(String[] arguments) throws Exception {
        String mode = arguments[0];
        Path path = Path.of(arguments[1]);
        if (mode.equals("--create")) {
            if (Files.exists(path)) {
                throw new IllegalArgumentException("Fresh original path required");
            }
            try (Database database = new DatabaseBuilder(path)
                    .setFileFormat(Database.FileFormat.valueOf(arguments[2])).create()) {
                database.setEvaluateExpressions(false);
                Table table = new TableBuilder("Streams")
                        .addColumn(new ColumnBuilder("ID", DataType.LONG))
                        .addColumn(new ColumnBuilder("Short", DataType.TEXT)
                                .setLengthInUnits(255).setCompressedUnicode(true))
                        .addColumn(new ColumnBuilder("Memo", DataType.MEMO)
                                .setCompressedUnicode(true))
                        .toTable(database);
                for (String line : Files.readAllLines(Path.of(arguments[3]))) {
                    String[] fields = line.split("\t");
                    int length = HexFormat.of().parseHex(fields[1]).length;
                    String seed = "Q".repeat(length - 2);
                    table.addRow(Integer.parseInt(fields[0]), seed, seed);
                }
            }
            return;
        }

        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            Table table = database.getTable("Streams");
            if (mode.equals("--decode")) {
                ColumnImpl shortColumn = (ColumnImpl) table.getColumn("Short");
                ColumnImpl memoColumn = (ColumnImpl) table.getColumn("Memo");
                for (String line : Files.readAllLines(Path.of(arguments[2]))) {
                    String[] fields = line.split("\t");
                    byte[] stream = HexFormat.of().parseHex(fields[1]);
                    byte[] inline = ByteBuffer.allocate(12 + stream.length)
                            .order(ByteOrder.LITTLE_ENDIAN)
                            .putInt(0x80000000 | stream.length).putLong(0).put(stream).array();
                    System.out.printf("D\t%s\t%s\t%s%n", fields[0],
                            hex((String) shortColumn.read(stream)),
                            hex((String) memoColumn.read(inline)));
                }
            } else {
                for (Row row : table) {
                    RowIdImpl location = (RowIdImpl) row.getId();
                    System.out.printf("R\t%d\t%d\t%d\t%s\t%s%n", row.getInt("ID"),
                            location.getPageNumber(), location.getRowNumber(),
                            hex(row.getString("Short")), hex(row.getString("Memo")));
                }
            }
        }
    }
}
