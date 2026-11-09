import com.healthmarketscience.jackcess.*;
import java.nio.file.*;
import java.util.*;

// Original inert fixed-field update recipe and read-only independent inspector.
public class UpdateFixture {
  public static void main(String[] args) throws Exception {
    if(args.length!=2)throw new IllegalArgumentException("Expected fresh path and V2000/V2007/V2016/V2019, or --inspect path");
    if(args[0].equals("--inspect")){inspect(Path.of(args[1]));return;}
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format=switch(args[1]){case "V2000"->Database.FileFormat.V2000;case "V2007"->Database.FileFormat.V2007;case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      db.setEvaluateExpressions(false);
      Table table=new TableBuilder("Items")
        .addColumn(new ColumnBuilder("Id",DataType.LONG))
        .addColumn(new ColumnBuilder("Score",DataType.LONG).putProperty("Required",true).putProperty("ValidationRule",">=-100 And <=100"))
        .addColumn(new ColumnBuilder("Notes",DataType.MEMO))
        .addColumn(new ColumnBuilder("Payload",DataType.OLE))
        .addIndex(new IndexBuilder("PK_Items").addColumns("Id").setPrimaryKey()).toTable(db);
      for(int id=1;id<=3;id++) {
        byte[] payload=new byte[12_288];Arrays.fill(payload,(byte)(id+160));
        table.addRow(id,id*10,"Unicode é🦀 ".repeat(700),payload);
      }
    }
    inspect(path);
  }
  static void inspect(Path path)throws Exception {
    try(Database db=new DatabaseBuilder(path).setReadOnly(true).open()) {
      for(Row row:db.getTable("Items")) {
        System.out.printf("%d\t%d\t%d\t%d%n",row.getInt("Id"),row.getInt("Score"),row.getString("Notes").length(),row.getBytes("Payload").length);
      }
    }
  }
}
