import com.healthmarketscience.jackcess.*;
import java.nio.file.*;

// Original repository-licensed exact composite/null relationship fixture.
public class CompositeConstraintFixture {
  interface Attempt{void run()throws Exception;}
  static void rejects(Attempt action)throws Exception{try{action.run();}catch(ConstraintViolationException expected){return;}throw new IllegalStateException("Expected constraint rejection");}
  public static void main(String[] args)throws Exception{
    if(args.length!=2)throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output must not exist");
    var format=switch(args[1]){case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()){
      Table p=new TableBuilder("Parents").addColumn(new ColumnBuilder("A",DataType.LONG)).addColumn(new ColumnBuilder("B",DataType.LONG)).addIndex(new IndexBuilder("Pair").addColumns("A","B").setUnique()).toTable(db);
      Table c=new TableBuilder("Children").addColumn(new ColumnBuilder("X",DataType.LONG)).addColumn(new ColumnBuilder("Y",DataType.LONG)).toTable(db);
      new RelationshipBuilder(p,c).addColumns("A","X").addColumns("B","Y").setReferentialIntegrity().setName("Pairs").toRelationship(db);
      p.addRow(1,10);p.addRow(2,20);p.addRow(null,30);p.addRow(4,null);p.addRow(null,null);p.addRow(null,null);
      c.addRow(1,10);c.addRow(2,20);c.addRow(null,30);c.addRow(4,null);c.addRow(null,null);c.addRow(null,null);
      rejects(()->c.addRow(1,20));rejects(()->c.addRow(null,10));rejects(()->c.addRow(1,null));rejects(()->p.addRow(null,30));
      Table unique=new TableBuilder("UniqueChildren").addColumn(new ColumnBuilder("X",DataType.LONG)).addColumn(new ColumnBuilder("Y",DataType.LONG)).addIndex(new IndexBuilder("Pair").addColumns("X","Y").setUnique()).toTable(db);
      new RelationshipBuilder(p,unique).addColumns("A","X").addColumns("B","Y").setReferentialIntegrity().setName("UniquePairs").toRelationship(db);
      unique.addRow(null,30);unique.addRow(null,null);unique.addRow(null,null);rejects(()->unique.addRow(null,30));
    }
    System.out.println("All original composite/null acceptance and rejection assertions passed.");
  }
}
