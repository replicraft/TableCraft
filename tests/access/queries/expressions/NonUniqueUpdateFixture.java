import com.healthmarketscience.jackcess.*;
import java.nio.file.*;
import java.util.Arrays;

/** Original inert non-unique mutation recipe; creates only fresh independent inputs. */
public class NonUniqueUpdateFixture {
  static Table table(Database db,String name,boolean unique,boolean rule)throws Exception {
    var score=new ColumnBuilder("Score",DataType.LONG);
    if(rule)score.putProperty("ValidationRule",">=0");
    var builder=new TableBuilder(name).addColumn(new ColumnBuilder("Id",DataType.LONG))
      .addColumn(score).addColumn(new ColumnBuilder("Notes",DataType.MEMO))
      .addColumn(new ColumnBuilder("Payload",DataType.OLE))
      .addIndex(new IndexBuilder("Primary").addColumns("Id").setPrimaryKey())
      .addIndex(new IndexBuilder("Asc").addColumns("Score"))
      .addIndex(new IndexBuilder("Desc").addColumns(false,"Score"))
      .addIndex(new IndexBuilder("Alias").addColumns("Score"));
    if(unique)builder.addIndex(new IndexBuilder("UniqueGuard").addColumns("Score").setUnique());
    return builder.toTable(db);
  }
  static void row(Table table,int id,Integer score,int ordinal)throws Exception {
    byte[] payload=new byte[ordinal==0?12288:16];Arrays.fill(payload,(byte)(ordinal%256));
    table.addRow(id,score,ordinal==0?"Unicode é🦀 ".repeat(700):"row "+ordinal,payload);
  }
  public static void main(String[] args)throws Exception {
    if(args.length!=2)throw new IllegalArgumentException("Expected fresh path and V2000/V2016/V2019");
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output exists");
    Database.FileFormat format=switch(args[1]){case "V2000"->Database.FileFormat.V2000;case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      db.setEvaluateExpressions(false);
      Table leaves=table(db,"Leaves",false,false);
      int[] ids={70,20,60,10,50,30,40};Integer[] scores={null,10,20,10,30,20,null};
      for(int i=0;i<ids.length;i++)row(leaves,ids[i],scores[i],i);
      Table tree=table(db,"Tree",false,false);
      for(int i=0;i<1000;i++)row(tree,(i*37)%1000,i%17==0?null:i%31,i);
      for(String name:new String[]{"Unique","Rules"}) {
        Table guard=table(db,name,name.equals("Unique"),name.equals("Rules"));
        for(int i=0;i<3;i++)row(guard,i+1,(i+1)*10,i);
      }
    }
    System.out.println("Created fresh inert duplicate/NULL/alias/node update inputs.");
  }
}
