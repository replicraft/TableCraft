import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.impl.TableImpl;
import java.nio.file.*;

// Original inert originals. The independent Python recipe removes declarations on copies.
public class FixedPrefixHistoryFixture {
  public static void main(String[] args) throws Exception {
    if (args.length != 2 || Files.exists(Path.of(args[0]))) {
      throw new IllegalArgumentException("Expected fresh path and format");
    }
    Database.FileFormat format = Database.FileFormat.valueOf(args[1]);
    try (Database db = new DatabaseBuilder(Path.of(args[0])).setFileFormat(format).create()) {
      db.setEvaluateExpressions(false);
      for (boolean variableFirst : new boolean[] {false, true}) {
        String name = variableFirst ? "VariableFirst" : "FixedFirst";
        Table table = new TableBuilder(name)
            .addColumn(new ColumnBuilder("ID", DataType.LONG)).toTable(db);
        table.addRow(1);
        if (variableFirst) {
          new ColumnBuilder("DropText", DataType.TEXT).setLengthInUnits(40).addToTable(table);
          table.addRow(2, "removed é");
          new ColumnBuilder("DropFixed", DataType.LONG).addToTable(table);
          table.addRow(3, "removed é", 33);
        } else {
          new ColumnBuilder("DropFixed", DataType.LONG).addToTable(table);
          table.addRow(2, 22);
          new ColumnBuilder("DropText", DataType.TEXT).setLengthInUnits(40).addToTable(table);
          table.addRow(3, 33, "removed é");
        }
        new ColumnBuilder("KeepText", DataType.TEXT).setLengthInUnits(40).addToTable(table);
        new ColumnBuilder("Flag", DataType.BOOLEAN).addToTable(table);
        new ColumnBuilder("Counter", DataType.LONG).addToTable(table);
        table.addRow(variableFirst
            ? new Object[] {4, "removed", 44, "kept é🦀", true, 400}
            : new Object[] {4, 44, "removed", "kept é🦀", true, 400});
        System.out.println(name + "\t" + ((TableImpl) table).getTableDefPageNumber());
      }
    }
  }
}
