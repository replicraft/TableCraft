import com.healthmarketscience.jackcess.*;
import java.nio.file.*;

/** Original inert fixtures for reciprocal-index links; no application execution. */
public class ForeignIndexFixture {
  static Table parent(Database db,String name)throws Exception {
    Table table=new TableBuilder(name).addColumn(new ColumnBuilder("Id",DataType.LONG))
      .addIndex(new IndexBuilder("Primary").addColumns("Id").setPrimaryKey()).toTable(db);
    table.addRow(1);return table;
  }
  static Table child(Database db,String name)throws Exception {
    Table table=new TableBuilder(name).addColumn(new ColumnBuilder("Id",DataType.LONG))
      .addColumn(new ColumnBuilder("Parent",DataType.LONG))
      .addIndex(new IndexBuilder("Primary").addColumns("Id").setPrimaryKey()).toTable(db);
    table.addRow(1,1);table.addRow(2,null);return table;
  }
  public static void main(String[] args)throws Exception {
    if(args.length!=2)throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output exists");
    Database.FileFormat format=switch(args[1]){case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      db.setEvaluateExpressions(false);
      for(String mode:new String[]{"Restrict","Cascade","SetNull"}) {
        Table parent=parent(db,mode+"Parents"),child=child(db,mode+"Children");
        var relation=new RelationshipBuilder(parent,child).addColumns("Id","Parent").setReferentialIntegrity().setName(mode+"Link");
        if(mode.equals("Cascade"))relation.setCascadeUpdates().setCascadeDeletes();
        if(mode.equals("SetNull"))relation.setCascadeNullOnDelete();
        relation.toRelationship(db);
        if(mode.equals("Restrict")) {
          Table second=child(db,"SecondChildren");
          new RelationshipBuilder(parent,second).addColumns("Id","Parent").setReferentialIntegrity().setName("SecondLink").toRelationship(db);
        }
      }
      Table parent=new TableBuilder("CompositeParents").addColumn(new ColumnBuilder("A",DataType.LONG))
        .addColumn(new ColumnBuilder("B",DataType.LONG)).addIndex(new IndexBuilder("Primary").addColumns("A","B").setPrimaryKey()).toTable(db);
      Table child=new TableBuilder("CompositeChildren").addColumn(new ColumnBuilder("X",DataType.LONG))
        .addColumn(new ColumnBuilder("Y",DataType.LONG)).toTable(db);
      parent.addRow(1,2);child.addRow(1,2);
      new RelationshipBuilder(parent,child).addColumns("A","X").addColumns("B","Y").setReferentialIntegrity().setName("CompositeLink").toRelationship(db);
      Table self=new TableBuilder("SelfLinks").addColumn(new ColumnBuilder("Id",DataType.LONG))
        .addColumn(new ColumnBuilder("Parent",DataType.LONG)).addIndex(new IndexBuilder("Primary").addColumns("Id").setPrimaryKey()).toTable(db);
      self.addRow(1,null);self.addRow(2,1);
      new RelationshipBuilder(self,self).addColumns("Id","Parent").setReferentialIntegrity().setName("SelfLink").toRelationship(db);
    }
    System.out.println("Created fresh inert reciprocal-link fixture.");
  }
}
