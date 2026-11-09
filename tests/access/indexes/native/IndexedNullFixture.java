import com.healthmarketscience.jackcess.*;
import java.nio.file.*;
import java.util.Arrays;

/** Original inert indexed-NULL recipe; only fresh independent ACE 16 inputs. */
public class IndexedNullFixture {
  static Table table(Database db,String name,boolean unique,boolean required,boolean rule)throws Exception {
    var score=new ColumnBuilder("Score",DataType.LONG);
    if(rule)score.putProperty("ValidationRule","Is Not Null");
    var builder=new TableBuilder(name).addColumn(new ColumnBuilder("Id",DataType.LONG))
      .addColumn(score).addColumn(new ColumnBuilder("Notes",DataType.MEMO))
      .addColumn(new ColumnBuilder("Payload",DataType.OLE))
      .addIndex(new IndexBuilder("Primary").addColumns("Id").setPrimaryKey());
    for(String index:new String[]{"Asc","Desc","Alias"}) {
      var key=new IndexBuilder(index).addColumns(!index.equals("Desc"),"Score");
      if(unique)key.setUnique();if(required)key.setRequired();builder.addIndex(key);
    }
    return builder.toTable(db);
  }
  static void row(Table table,int id,Integer score,int ordinal)throws Exception {
    byte[] payload=new byte[ordinal==0?12288:16];Arrays.fill(payload,(byte)(ordinal%256));
    table.addRow(id,score,ordinal==0?"Unicode é🦀 ".repeat(700):"row "+ordinal,payload);
  }
  public static void main(String[] args)throws Exception {
    if(args.length!=2)throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output exists");
    Database.FileFormat format=switch(args[1]){case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      db.setEvaluateExpressions(false);
      Table ordinary=table(db,"Ordinary",false,false,false);
      int[] ids={70,20,60,10,50,30,40};Integer[] scores={null,10,20,10,30,20,null};
      for(int i=0;i<ids.length;i++)row(ordinary,ids[i],scores[i],i);
      Table tree=table(db,"Tree",false,false,false);
      for(int i=0;i<1000;i++)row(tree,(i*37)%1000,i%17==0?null:i%31,i);
      Table unique=table(db,"Unique",true,false,false);
      Integer[] keys={null,10,null,20,30};
      for(int i=0;i<keys.length;i++)row(unique,50-i*10,keys[i],i);
      for(String name:new String[]{"Required","Rules"}) {
        Table guard=table(db,name,false,name.equals("Required"),name.equals("Rules"));
        for(int i=0;i<3;i++)row(guard,i+1,(i+1)*10,i);
      }
    }
    System.out.println("Created fresh inert indexed-NULL/unique/required/node controls.");
  }
}
