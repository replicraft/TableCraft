import com.healthmarketscience.jackcess.*;
import java.nio.file.Path;
public class EvolutionFixture {
  public static void main(String[] args) throws Exception {
    if(java.nio.file.Files.exists(Path.of(args[0]))) throw new IllegalArgumentException("Output must not exist");
    try(Database db=new DatabaseBuilder(Path.of(args[0])).setFileFormat(Database.FileFormat.V2007).create()) {
      Table table=new TableBuilder("Evolved")
        .addColumn(new ColumnBuilder("ID",DataType.LONG))
        .toTable(db);
      table.addRow(1);
      new ColumnBuilder("Label",DataType.TEXT).setLengthInUnits(20).addToTable(table);
      table.addRow(2,"middle");
      new ColumnBuilder("Counter",DataType.LONG).putProperty("DefaultValue","999").addToTable(table);
      new ColumnBuilder("Flag",DataType.BOOLEAN).putProperty("DefaultValue","True").addToTable(table);
      new ColumnBuilder("Notes",DataType.MEMO).addToTable(table);
      table.addRow(3,"after",42,true,"appended");
    }
  }
}
