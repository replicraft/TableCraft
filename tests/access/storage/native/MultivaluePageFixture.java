import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.complex.ComplexValueForeignKey;
import java.nio.file.*;

/** Original mixed scalar/collection/cache recipe. No expression evaluation. */
public class MultivaluePageFixture {
  static TableBuilder builder(String name) {
    return new TableBuilder(name)
      .addColumn(new ColumnBuilder("ID", DataType.LONG).setAutoNumber(true))
      .addColumn(new ColumnBuilder("Label", DataType.TEXT).setLengthInUnits(40))
      .addColumn(new ColumnBuilder("Numbers").setMultiValue(DataType.LONG).setValueList("1", "2"))
      .addColumn(new ColumnBuilder("Words").setMultiValue(DataType.TEXT).setValueList("Red", "Blue"))
      .addColumn(new ColumnBuilder("Derived", DataType.LONG).setCalculatedInfo("[ID]*2"));
  }
  public static void main(String[] args) throws Exception {
    if(args.length != 2) throw new IllegalArgumentException("Expected fresh output and V2016/V2019");
    Path path = Path.of(args[0]);
    if(Files.exists(path)) throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format = switch(args[1]) {
      case "V2016" -> Database.FileFormat.V2016;
      case "V2019" -> Database.FileFormat.V2019;
      default -> throw new IllegalArgumentException("Unsupported format");
    };
    try(Database db = new DatabaseBuilder(path).setFileFormat(format).create()) {
      db.setEvaluateExpressions(false);
      Table table = builder("MixedValues").toTable(db);
      Object[][] cells = {{Column.AUTO_NUMBER, "first", Column.AUTO_NUMBER, Column.AUTO_NUMBER, 41},
                          {Column.AUTO_NUMBER, "", Column.AUTO_NUMBER, Column.AUTO_NUMBER, null},
                          {Column.AUTO_NUMBER, null, Column.AUTO_NUMBER, Column.AUTO_NUMBER, -7}};
      for(int i = 0; i < cells.length; i++) {
        Object[] row = table.addRow(cells[i]);
        ComplexValueForeignKey numbers = (ComplexValueForeignKey)table.getColumn("Numbers").getRowValue(row);
        ComplexValueForeignKey words = (ComplexValueForeignKey)table.getColumn("Words").getRowValue(row);
        if(i == 0) {
          numbers.addMultiValue(1); numbers.addMultiValue(null); numbers.addMultiValue(1);
          words.addMultiValue("Red"); words.addMultiValue("é🦀\t\n");
        } else if(i == 2) {
          numbers.addMultiValue(100); words.addMultiValue(null);
        }
      }
      builder("EmptyMixedValues").toTable(db);
    }
  }
}
