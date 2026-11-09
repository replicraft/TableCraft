import com.healthmarketscience.jackcess.*;
import java.nio.file.*;

// Original inert rule recipe; expected write rejections are checked by Jackcess.
public class ValidationFixture {
  interface Attempt { void run() throws Exception; }
  static void rejects(Attempt attempt) throws Exception {
    try { attempt.run(); } catch (InvalidValueException expected) { return; }
    throw new IllegalStateException("Expected rule rejection");
  }
  public static void main(String[] args) throws Exception {
    if(args.length!=2)throw new IllegalArgumentException("Expected fresh path and V2000/V2007/V2016/V2019, or --inspect path");
    if(args[0].equals("--inspect")){inspect(Path.of(args[1]));return;}
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format=switch(args[1]){case "V2000"->Database.FileFormat.V2000;case "V2007"->Database.FileFormat.V2007;case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      Table range=new TableBuilder("Ranges").addColumn(new ColumnBuilder("Score",DataType.LONG)
        .putProperty("Required",true).putProperty("ValidationRule",">=0 And <=100")).toTable(db);
      range.addRow(0);range.addRow(7);range.addRow(100);
      rejects(()->range.addRow(-1));rejects(()->range.addRow(101));
      Table bounds=new TableBuilder("Bounds").putProperty("ValidationRule","[Minimum] <= [Maximum]")
        .addColumn(new ColumnBuilder("Minimum",DataType.LONG)).addColumn(new ColumnBuilder("Maximum",DataType.LONG)).toTable(db);
      bounds.addRow(1,2);bounds.addRow(3,3);rejects(()->bounds.addRow(3,2));
      Table nullable=new TableBuilder("Nullable").addColumn(new ColumnBuilder("Score",DataType.LONG)
        .putProperty("Required",false).putProperty("ValidationRule","Is Null Or >=0")).toTable(db);
      nullable.addRow(1);nullable.addRow((Object)null);
      Table calls=new TableBuilder("Calls").addColumn(new ColumnBuilder("Label",DataType.TEXT)
        .putProperty("ValidationRule","Len([Label]) > 0")).toTable(db);calls.addRow("ok");
      Table unknown=new TableBuilder("UnknownNull").addColumn(new ColumnBuilder("Score",DataType.LONG)).toTable(db);
      unknown.addRow((Object)null);
      // Deliberately attach a rule after inserting NULL; this existing state is
      // an uncertainty probe, not a claim that normal validated writes accept it.
      unknown.getColumn("Score").getProperties().put("ValidationRule",">=0");
      unknown.getColumn("Score").getProperties().save();
      new TableBuilder("EmptyCalls").addColumn(new ColumnBuilder("Label",DataType.TEXT)
        .putProperty("ValidationRule","Len([Label]) > 0")).toTable(db);
    }
    inspect(path);
  }
  static void inspect(Path path)throws Exception {
    try(Database db=new DatabaseBuilder(path).setReadOnly(true).open()) {
      for(String name:new String[]{"Ranges","Bounds","Nullable","Calls","EmptyCalls","UnknownNull"}) {
        System.out.printf("%s\t%d%n",name,db.getTable(name).getRowCount());
      }
    }
  }
}
