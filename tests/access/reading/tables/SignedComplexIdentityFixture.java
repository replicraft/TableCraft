import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import com.healthmarketscience.jackcess.complex.ComplexValueForeignKey;
import com.healthmarketscience.jackcess.impl.complex.ComplexColumnInfoImpl;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.LocalDateTime;
import java.util.HashMap;

/** Original inert fixture: persisted signed child IDs, positive owner keys. */
public class SignedComplexIdentityFixture {
    public static void main(String[] arguments) throws Exception {
        Path path = Path.of(arguments[0]);
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Fresh output path required");
        }
        try (Database database = new DatabaseBuilder(path)
                .setFileFormat(Database.FileFormat.valueOf(arguments[1])).create()) {
            database.setEvaluateExpressions(false);
            Table table = new TableBuilder("Identities")
                    .addColumn(new ColumnBuilder("ID", DataType.LONG))
                    .addColumn(new ColumnBuilder("Values")
                            .setMultiValue(DataType.LONG).setValueList("1", "2"))
                    .addColumn(new ColumnBuilder("Files").setAttachment())
                    .addColumn(new ColumnBuilder("Notes", DataType.MEMO).setAppendOnly(true))
                    .toTable(database);
            Object[] inserted = table.addRow(1);
            ComplexValueForeignKey numbers =
                    (ComplexValueForeignKey) table.getColumn("Values").getRowValue(inserted);
            numbers.addMultiValue(42);
            ComplexValueForeignKey files =
                    (ComplexValueForeignKey) table.getColumn("Files").getRowValue(inserted);
            files.addAttachment("", "id=1.bin", "bin", new byte[] {1, 2, 3},
                    (LocalDateTime) null, 0);
            String hidden = table.getColumn("Notes").getVersionHistoryColumn().getName();
            ComplexValueForeignKey versions =
                    (ComplexValueForeignKey) table.getColumn(hidden).getRowValue(inserted);
            versions.addVersion("id=1", LocalDateTime.of(2024, 1, 1, 0, 0));
            table.addRow(2);

            for (String name : new String[] {"Values", "Files", hidden}) {
                ComplexColumnInfoImpl<?> info =
                        (ComplexColumnInfoImpl<?>) table.getColumn(name).getComplexInfo();
                Table flat = info.getPrimaryKeyColumn().getTable();
                flat.setAllowAutoNumberInsert(true);
                Row base = flat.iterator().next();
                for (int identity : new int[] {
                        Integer.MIN_VALUE, -7, -2, -1, 0, 7, Integer.MAX_VALUE}) {
                    HashMap<String, Object> child = new HashMap<>(base);
                    child.put(info.getPrimaryKeyColumn().getName(), identity);
                    if (name.equals("Values")) {
                        child.put("Value", identity == -1 ? null : identity);
                    } else if (name.equals("Files")) {
                        child.put("FileName", "id=" + identity + ".bin");
                    } else {
                        child.put("Notes", "id=" + identity);
                    }
                    flat.addRowFromMap(child);
                }
            }
        }
    }
}
