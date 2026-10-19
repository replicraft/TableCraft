import com.healthmarketscience.jackcess.*;
import java.nio.file.*;

/** Original inert fixed-GUID index recipe; fresh outputs and canonical input only. */
public class GuidIndexFixture {
  interface Attempt {void run()throws Exception;}
  static void rejects(Attempt action)throws Exception {
    try{action.run();}catch(ConstraintViolationException expected){return;}
    throw new AssertionError("Expected constraint rejection");
  }
  public static void main(String[] args)throws Exception {
    if(args.length!=2)throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format=switch(args[1]){case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    String[] keys={"{00000000-0000-0000-0000-000000000000}","{FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF}",
      "{80000000-0000-0000-0000-000000000000}","{00000001-0000-0000-0000-000000000000}",
      "{01000000-0000-0000-0000-000000000000}","{00000000-0001-0000-0000-000000000000}",
      "{00000000-0100-0000-0000-000000000000}","{00000000-0000-0001-0000-000000000000}",
      "{00000000-0000-0100-0000-000000000000}","{00000000-0000-0000-0001-000000000000}",
      "{00000000-0000-0000-0100-000000000000}","{00000000-0000-0000-0000-000000000001}",
      "{00000000-0000-0000-0000-010000000000}","{6B29FC40-CA47-1067-B31D-00DD010662DA}",
      "{1234ABCD-89EF-0123-4567-89ABCDEF0123}","{80808080-8080-8080-8080-808080808080}"};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      Table table=new TableBuilder("GuidIndexed").addColumn(new ColumnBuilder("ID",DataType.LONG))
        .addColumn(new ColumnBuilder("Value",DataType.GUID))
        .addIndex(new IndexBuilder("PrimaryKey").addColumns("ID").setPrimaryKey())
        .addIndex(new IndexBuilder("GuidAsc").addColumns("Value"))
        .addIndex(new IndexBuilder("GuidDesc").addColumns(false,"Value")).toTable(db);
      for(int i=0;i<1000;i++)table.addRow(i,i%11==0?null:keys[(i*7)%keys.length]);
      Table unique=new TableBuilder("GuidUnique").addColumn(new ColumnBuilder("Value",DataType.GUID))
        .addIndex(new IndexBuilder("UniqueGuid").addColumns("Value").setUnique()).toTable(db);
      for(String key:keys)unique.addRow(key);
      unique.addRow((Object)null);unique.addRow((Object)null);
      rejects(()->unique.addRow(keys[0]));
      Table primary=new TableBuilder("GuidPrimary").addColumn(new ColumnBuilder("Value",DataType.GUID))
        .addIndex(new IndexBuilder("PrimaryGuid").addColumns("Value").setPrimaryKey()).toTable(db);
      for(String key:keys)primary.addRow(key);
      rejects(()->primary.addRow((Object)null));rejects(()->primary.addRow(keys[0]));
      new TableBuilder("GuidEmpty").addColumn(new ColumnBuilder("Value",DataType.GUID))
        .addIndex(new IndexBuilder("EmptyGuid").addColumns("Value")).toTable(db);
      new TableBuilder("GuidCompound").addColumn(new ColumnBuilder("Value",DataType.GUID))
        .addColumn(new ColumnBuilder("ID",DataType.LONG))
        .addIndex(new IndexBuilder("CompoundGuid").addColumns("Value","ID")).toTable(db);
    }
    System.out.println("GUID unique/primary acceptance and rejection assertions passed.");
  }
}
