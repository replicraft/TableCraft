import com.healthmarketscience.jackcess.*;
import java.nio.file.*;
import java.util.*;

/** Original inert indexed GUID mutation recipe; fresh outputs only. */
public class GuidUpdateFixture {
  static final String A="{00112233-4455-6677-8899-AABBCCDDEEFF}";
  static final String B="{00112233-4455-6677-8899-AABBCCDDEE00}";
  static final String Z="{00000000-0000-0000-0000-000000000000}";
  static Table table(Database db,String name,boolean unique,boolean tuple) throws Exception {
    var builder=new TableBuilder(name).addColumn(new ColumnBuilder("Id",DataType.LONG))
      .addColumn(new ColumnBuilder("G",DataType.GUID));
    if(tuple) builder.addColumn(new ColumnBuilder("N",DataType.BIG_INT));
    builder.addColumn(new ColumnBuilder("Notes",DataType.MEMO)).addColumn(new ColumnBuilder("Payload",DataType.OLE));
    var asc=new IndexBuilder("Asc").addColumns("G");
    var desc=new IndexBuilder("Desc").addColumns(false,"G");
    if(tuple){asc.addColumns(false,"N");desc.addColumns("N");}
    if(unique){asc.setUnique();desc.setUnique();}
    return builder.addIndex(asc).addIndex(desc).addIndex(new IndexBuilder("Primary").addColumns("Id").setPrimaryKey()).toTable(db);
  }
  static void row(Table t,int id,String g,boolean tuple,long n) throws Exception {
    byte[] payload=new byte[id==0?12288:16];Arrays.fill(payload,(byte)id);
    String notes=id==0?"Unicode é🦀 ".repeat(700):"row "+id;
    if(tuple)t.addRow(id,g,n,notes,payload);else t.addRow(id,g,notes,payload);
  }
  public static void main(String[] args)throws Exception {
    if(args.length!=2 || Files.exists(Path.of(args[0]))) throw new IllegalArgumentException("Fresh path and format required");
    var format=switch(args[1]){case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Format");};
    try(Database db=new DatabaseBuilder(Path.of(args[0])).setFileFormat(format).create()) {
      db.setEvaluateExpressions(false);
      Table tree=table(db,"Tree",false,false);
      for(int i=0;i<1000;i++)row(tree,i,i%11==0?null:i%2==0?A:B,false,0);
      for(boolean tuple:new boolean[]{false,true}){
        String prefix=tuple?"Tuple":"";
        Table parents=table(db,prefix+"Parents",true,tuple),children=table(db,prefix+"Children",false,tuple);
        String[] keys={A,B,Z,null};
        for(int i=0;i<keys.length;i++) {row(parents,i,keys[i],tuple,i);row(children,i,keys[i],tuple,i);}
        row(children,4,A,tuple,0);
        var relation=new RelationshipBuilder(parents,children).addColumns("G","G").setReferentialIntegrity().setCascadeUpdates().setName(prefix+"Link");
        if(tuple)relation.addColumns("N","N");
        relation.toRelationship(db);
      }
      Table unique=table(db,"Unique",true,false);
      row(unique,0,A,false,0);row(unique,1,B,false,0);row(unique,2,null,false,0);row(unique,3,null,false,0);
    }
  }
}
