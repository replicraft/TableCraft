import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import java.nio.file.Files;
import java.nio.file.Path;

/** Original inert calculated Text/Memo family; stored caches deliberately differ from expression. */
public class CalculatedTextFamilyFixture {
    private static Table table(
        Database database, String name, DataType type, boolean compressed, String family
    ) throws Exception {
        ColumnBuilder cache = new ColumnBuilder("Cached", type)
            .setCalculatedInfo("\"recomputed\"").setCompressedUnicode(compressed)
            .setHyperlink(family.equals("Hyperlink"));
        if (type == DataType.TEXT) {
            cache.setLengthInUnits(200);
        }
        if (family.equals("RichText")) {
            cache.putProperty("TextFormat", DataType.BYTE, (byte) 1);
        }
        return new TableBuilder(name).addColumn(new ColumnBuilder("ID", DataType.LONG))
            .addColumn(cache).toTable(database);
    }

    private static String first(String family) {
        return switch (family) {
            case "Plain" -> "ASCII";
            case "Hyperlink" -> "Display#https://example.invalid/a#part#é🦀";
            case "RichText" -> "<div><b>ASCII</b>&amp;literal</div>";
            default -> throw new IllegalArgumentException("Unknown original Text family");
        };
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 2) {
            throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
        }
        Path path = Path.of(args[0]);
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Fresh output required");
        }
        Database.FileFormat format = switch (args[1]) {
            case "V2016" -> Database.FileFormat.V2016;
            case "V2019" -> Database.FileFormat.V2019;
            default -> throw new IllegalArgumentException("Unsupported profile");
        };
        try (Database database = new DatabaseBuilder(path).setFileFormat(format).create()) {
            database.setEvaluateExpressions(false);
            for (DataType type : new DataType[] {DataType.TEXT, DataType.MEMO}) {
                String[] families = type == DataType.TEXT
                    ? new String[] {"Plain"} : new String[] {"Plain", "Hyperlink", "RichText"};
                for (String family : families) {
                    for (boolean compressed : new boolean[] {false, true}) {
                        String name = (type == DataType.TEXT ? "Short" : "Memo") + family
                            + (compressed ? "RequestedCompression" : "Wide");
                        Table table = table(database, name, type, compressed, family);
                        String large = type == DataType.TEXT ? "x".repeat(190) + "é🦀" : "é🦀".repeat(3000);
                        String[] values = {
                            first(family), "", null, "é🦀\0\t\"\\\n", large,
                            "literal\uFEFFsuffix", "unchanged stale cache", "late-" + name,
                        };
                        for (int index = 0; index < values.length; index++) {
                            table.addRow(index + 1, values[index]);
                        }
                        table(database, "Empty" + name, type, compressed, family);
                    }
                }
            }
        }
        System.out.println("Original cached Text/Memo families created with expression evaluation disabled");
    }
}
