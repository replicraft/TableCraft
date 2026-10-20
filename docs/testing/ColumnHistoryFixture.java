import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.impl.TableImpl;
import java.nio.file.*;

// Original inert append-only inputs; the Python recipe removes declarations on copies.
public class ColumnHistoryFixture {
  public static void main(String[] args) throws Exception {
    if(args.length!=2 || Files.exists(Path.of(args[0]))) throw new IllegalArgumentException("Expected fresh path and format");
    Database.FileFormat format=Database.FileFormat.valueOf(args[1]);
    try(Database db=new DatabaseBuilder(Path.of(args[0])).setFileFormat(format).create()) {
      db.setEvaluateExpressions(false);
      for(String name:new String[]{"FixedGap","VariableGap","MixedGap","AmbiguousGap"}) {
        Table table=new TableBuilder(name).addColumn(new ColumnBuilder("ID",DataType.LONG)).toTable(db);
        table.addRow(1);
        new ColumnBuilder("DropFixed",DataType.LONG).addToTable(table);
        if(!name.equals("MixedGap")) table.addRow(2,22);
        new ColumnBuilder("DropText",DataType.TEXT).setLengthInUnits(40).addToTable(table);
        table.addRow(3,33,"removed");
        new ColumnBuilder("KeepText",DataType.TEXT).setLengthInUnits(40).addToTable(table);
        table.addRow(4,44,"removed","four é🦀");
        new ColumnBuilder("Flag",DataType.BOOLEAN).addToTable(table);
        table.addRow(5,55,"removed","",true);
        new ColumnBuilder("Counter",DataType.LONG).addToTable(table);
        table.addRow(6,66,"removed",null,false,600);
        System.out.println(name+"\t"+((TableImpl)table).getTableDefPageNumber());
      }
    }
  }
}
