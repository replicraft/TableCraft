import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.complex.*;
import java.nio.file.*;

/** Original mixed schema with keys physically after unavailable collection fields. */
public class MixedIndexFixture {
  public static void main(String[] args) throws Exception {
    if(args.length != 2) throw new IllegalArgumentException("Expected fresh file and V2007/V2016/V2019");
    Path path = Path.of(args[0]); if(Files.exists(path)) throw new IllegalArgumentException("Output exists");
    var format = switch(args[1]) { case "V2007" -> Database.FileFormat.V2007; case "V2016" -> Database.FileFormat.V2016; case "V2019" -> Database.FileFormat.V2019; default -> throw new IllegalArgumentException("Unsupported format"); };
    boolean wide = format != Database.FileFormat.V2007;
    try(Database db = new DatabaseBuilder(path).setFileFormat(format).create()) {
      db.setEvaluateExpressions(false);
      var builder = new TableBuilder("MixedIndexed")
        .addColumn(new ColumnBuilder("Choices").setMultiValue(DataType.TEXT).setValueList("Red", "Blue"))
        .addColumn(new ColumnBuilder("ID", DataType.LONG))
        .addColumn(new ColumnBuilder("Files").setAttachment())
        .addColumn(new ColumnBuilder("Notes", DataType.MEMO).setAppendOnly(true))
        .addColumn(new ColumnBuilder("Score", DataType.LONG))
        .addColumn(new ColumnBuilder("Label", DataType.TEXT).setLengthInUnits(30))
        .addIndex(new IndexBuilder("IdAsc").addColumns("ID"))
        .addIndex(new IndexBuilder("IdDesc").addColumns(false, "ID"))
        .addIndex(new IndexBuilder("IdScore").addColumns("ID", "Score"))
        .addIndex(new IndexBuilder("ScoreKey").addColumns("Score").setUnique())
        .addIndex(new IndexBuilder("SkipNull").addColumns("ID").setIgnoreNulls())
        .addIndex(new IndexBuilder("LabelKey").addColumns("Label"));
      if(wide) builder.addColumn(new ColumnBuilder("Big", DataType.BIG_INT))
        .addIndex(new IndexBuilder("BigAsc").addColumns("Big"))
        .addIndex(new IndexBuilder("BigDesc").addColumns(false, "Big"));
      Table table = builder.toTable(db);
      Integer[] narrow = {null, Integer.MIN_VALUE, -40, -40, -1, 0, 1, 1, 20, Integer.MAX_VALUE};
      Long[] big = {null, Long.MIN_VALUE, -9007199254740993L, -1L, 0L, 1L, 9007199254740993L, 9007199254740993L, 9007199254740994L, Long.MAX_VALUE};
      for(int i = 0; i < 1000; ++i) {
        Integer id;
        if(i < narrow.length) id = narrow[i];
        else if(i % 113 == 0) id = null;
        else id = (i * 37) % 1000 - 500;
        Long large;
        if(i < big.length) large = big[i];
        else large = 9007199254740993L + 11L * i;
        Object[] values = wide
          ? new Object[]{Column.AUTO_NUMBER, id, Column.AUTO_NUMBER, "note é " + i, i, "row" + i, large}
          : new Object[]{Column.AUTO_NUMBER, id, Column.AUTO_NUMBER, "note é " + i, i, "row" + i};
        table.addRow(values);
      }
    }
  }
}
