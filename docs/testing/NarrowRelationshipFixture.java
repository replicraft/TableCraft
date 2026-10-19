import com.healthmarketscience.jackcess.*;
import java.nio.file.*;

/** Original inert exact narrow-integer relationship recipe. */
public class NarrowRelationshipFixture {
  interface Attempt {void run() throws Exception;}
  static void rejects(Attempt action)throws Exception {
    try{action.run();}catch(ConstraintViolationException expected){return;}
    throw new AssertionError("Expected constraint rejection");
  }
  static Table table(Database db,String name,DataType type,boolean unique)throws Exception {
    TableBuilder builder=new TableBuilder(name).addColumn(new ColumnBuilder("Key",type));
    if(unique)builder.addIndex(new IndexBuilder("UniqueKey").addColumns("Key").setUnique());
    return builder.toTable(db);
  }
  static Table pair(Database db,String name,boolean unique)throws Exception {
    TableBuilder builder=new TableBuilder(name).addColumn(new ColumnBuilder("A",DataType.BYTE)).addColumn(new ColumnBuilder("B",DataType.INT));
    if(unique)builder.addIndex(new IndexBuilder("Pair").addColumns("A","B").setUnique());
    return builder.toTable(db);
  }
  public static void main(String[] args)throws Exception {
    if(args.length!=2)throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format=switch(args[1]){case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      for(boolean small:new boolean[]{false,true}) {
        String label=small?"Short":"Byte";DataType type=small?DataType.INT:DataType.BYTE;
        Table parent=table(db,label+"Parents",type,true),child=table(db,label+"Children",type,false),unique=table(db,label+"UniqueChildren",type,true);
        new RelationshipBuilder(parent,child).addColumns("Key","Key").setReferentialIntegrity().setName(label+"Keys").toRelationship(db);
        new RelationshipBuilder(parent,unique).addColumns("Key","Key").setReferentialIntegrity().setName(label+"UniqueKeys").toRelationship(db);
        Integer[] values=small?new Integer[]{-32768,-32767,-1,0,1,32766,32767,null,null}:new Integer[]{0,1,127,128,254,255,null,null};
        for(Integer value:values){parent.addRow(value);child.addRow(value);}
        unique.addRow(values[0]);unique.addRow(values[1]);unique.addRow((Object)null);unique.addRow((Object)null);
        rejects(()->child.addRow(42));rejects(()->unique.addRow(values[0]));
      }
      Table parent=pair(db,"NarrowParents",true),child=pair(db,"NarrowChildren",false),unique=pair(db,"NarrowUniqueChildren",true);
      new RelationshipBuilder(parent,child).addColumns("A","A").addColumns("B","B").setReferentialIntegrity().setName("NarrowPairs").toRelationship(db);
      new RelationshipBuilder(parent,unique).addColumns("A","A").addColumns("B","B").setReferentialIntegrity().setName("NarrowUniquePairs").toRelationship(db);
      Object[][] pairs={{128,-32768},{255,32767},{0,0},{null,32767},{255,null},{null,null},{null,null}};
      for(Object[] values:pairs){parent.addRow(values);child.addRow(values);}
      unique.addRow(null,32767);unique.addRow(null,null);unique.addRow(null,null);
      rejects(()->child.addRow(128,32767));rejects(()->child.addRow(null,-32768));rejects(()->child.addRow(128,null));
      rejects(()->unique.addRow(null,32767));
    }
    System.out.println("All narrow key acceptance/rejection assertions passed.");
  }
}
