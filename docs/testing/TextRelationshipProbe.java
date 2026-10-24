import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.ConstraintViolationException;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.IndexBuilder;
import com.healthmarketscience.jackcess.RelationshipBuilder;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.charset.StandardCharsets;
import java.util.HexFormat;

// Original fresh-file capability probe. No imported expressions execute.
public class TextRelationshipProbe {
    private static ColumnBuilder column(String name, int version) {
        ColumnBuilder field = new ColumnBuilder(name, DataType.TEXT).setLengthInUnits(100);
        field.setTextSortOrder(version == 0
            ? ColumnImpl.GENERAL_LEGACY_SORT_ORDER : ColumnImpl.GENERAL_SORT_ORDER);
        return field;
    }

    private static void attempt(Table table, String kind, int version, String text) throws Exception {
        boolean accepted;
        try {
            table.addRow(text);
            accepted = true;
        } catch (ConstraintViolationException expected) {
            accepted = false;
        }
        String tag = text == null ? "N"
            : "T" + HexFormat.of().formatHex(text.getBytes(StandardCharsets.UTF_8));
        System.out.printf("%d\t%s\t%s\t%s%n", version, kind, tag, accepted);
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 2) {
            throw new IllegalArgumentException("Expected fresh path and existing format name");
        }
        Path path = Path.of(args[0]);
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Fresh output required");
        }
        try (Database database = new DatabaseBuilder(path)
            .setFileFormat(Database.FileFormat.valueOf(args[1])).create()) {
            database.setEvaluateExpressions(false);
            for (int version = 0; version <= 1; version++) {
                Table parent = new TableBuilder("Parents" + version)
                    .addColumn(column("Code", version))
                    .addIndex(new IndexBuilder("UniqueCode").addColumns("Code").setUnique())
                    .toTable(database);
                Table child = new TableBuilder("Children" + version)
                    .addColumn(column("Code", version)).toTable(database);
                new RelationshipBuilder(parent, child).addColumns("Code", "Code")
                    .setReferentialIntegrity().setName("TextKeys" + version).toRelationship(database);
                for (String text : new String[] {"Alpha", "é", "Straße", ""}) {
                    parent.addRow(text);
                }
                Table unique = new TableBuilder("UniqueProbe" + version)
                    .addColumn(column("Code", version))
                    .addIndex(new IndexBuilder("UniqueCode").addColumns("Code").setUnique())
                    .toTable(database);
                for (String text : new String[] {"Alpha", "é", "Straße", ""}) {
                    unique.addRow(text);
                }
                for (String text : new String[] {"alpha", "ALPHA", "Alpha ", "Alpha\0",
                    "É", "e\u0301", "Strasse", "", null, "e", "missing"}) {
                    attempt(child, "foreign", version, text);
                    attempt(unique, "unique", version, text);
                }
            }
        }
    }
}
