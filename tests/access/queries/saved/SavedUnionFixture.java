import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.query.Query;
import com.healthmarketscience.jackcess.impl.query.*;
import java.nio.ByteBuffer;
import java.nio.file.*;
import java.util.*;

/** Original inert SELECT/UNION metadata recipe; no SQL or application content runs. */
public class SavedUnionFixture {
  static Table catalog, queries;
  static int parent, nextId=Integer.MIN_VALUE+600;
  static void fragment(int id,byte attribute,String expression,Short flag,String name1,String name2,int order)throws Exception {
    queries.addRowFromMap(new QueryImpl.Row(null,attribute,expression,flag,null,name1,name2,id,ByteBuffer.allocate(4).putInt(order).array()).toTableRow());
  }
  static int start(String name,boolean union,short flags)throws Exception {
    int id=nextId++;
    Map<String,Object> row=new HashMap<>();
    row.put("Id",id);row.put("ParentId",parent);row.put("Name",name);row.put("Type",(short)5);row.put("Flags",union?128:0);
    catalog.addRowFromMap(row);
    fragment(id,(byte)1,null,(short)(union?9:1),null,null,1);
    fragment(id,(byte)3,null,flags,null,null,1);
    return id;
  }
  static int union(String name,String first,String second,short flags,String direction)throws Exception {
    int id=start(name,true,flags);
    fragment(id,(byte)5,first,null,null,"X7YZ_____1",1);
    fragment(id,(byte)5,second,null,null,"X7YZ_____2",2);
    if(direction!=null)fragment(id,(byte)11,"[N]",null,direction,null,1);
    return id;
  }
  public static void main(String[] args)throws Exception {
    if(args.length!=2)throw new IllegalArgumentException("Expected fresh path and V2000/V2007/V2016/V2019");
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output must not exist");
    var format=switch(args[1]){case "V2000"->Database.FileFormat.V2000;case "V2007"->Database.FileFormat.V2007;case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      db.setEvaluateExpressions(false);db.setAllowAutoNumberInsert(true);
      Table items=new TableBuilder("Items").addColumn(new ColumnBuilder("ID",DataType.LONG)).toTable(db);
      for(int id:new int[]{1,2,3})items.addRow(id);
      Table others=new TableBuilder("Others").addColumn(new ColumnBuilder("ID",DataType.LONG)).toTable(db);
      for(int id:new int[]{2,4})others.addRow(id);
      catalog=db.getSystemTable("MSysObjects");queries=db.getSystemTable("MSysQueries");
      for(Row row:catalog)if("Tables".equals(row.get("Name"))&&Objects.equals(row.get("Type"),(short)3))parent=row.getInt("Id");
      if(parent==0)throw new AssertionError("Missing query namespace container");
      int id=start("Leaf",false,(short)0);
      fragment(id,(byte)5,null,null,"Items",null,1);fragment(id,(byte)6,"[ID]",null,"N",null,1);
      fragment(id,(byte)8,"[ID] <= 2",null,null,null,1);
      union("UnionAll","SELECT ID AS N FROM Items WHERE ID >= 2","SELECT ID FROM Items WHERE ID <= 2",(short)0,"A");
      union("UnionDistinct","SELECT ID AS N FROM Items WHERE ID >= 2","SELECT ID FROM Items WHERE ID <= 2",(short)2,"D");
      union("UnionChain","SELECT ID AS N FROM Items WHERE ID=1","SELECT ID FROM Items WHERE ID=2 UNION SELECT ID FROM Items WHERE ID=1 UNION ALL SELECT ID FROM Items WHERE ID=3",(short)0,"A");
      union("UnionReferences","Leaf","TABLE Others",(short)0,"A");
      id=union("UnionParameter","SELECT ID AS N FROM Items WHERE ID >= Floor","SELECT ID FROM Others WHERE ID <= Floor",(short)0,"A");
      fragment(id,(byte)2,null,(short)4,"Floor",null,1);
      id=start("FromUnion",false,(short)0);
      fragment(id,(byte)5,null,null,"UnionAll","u",1);fragment(id,(byte)6,"[u].[N]",null,"Chosen",null,1);
      fragment(id,(byte)11,"[Chosen]",null,"D",null,1);
      union("UnionOwner","SELECT 1 AS N","SELECT 2",(short)4,null);
      union("UnionUdf","SELECT NeverRunMe() AS N","SELECT 2",(short)0,null);
      union("UnionCycle","UnionCycle","SELECT 1 AS N",(short)0,null);
      union("UnionMissing","NoSuchQuery","SELECT 1 AS N",(short)0,null);
      union("UnionComments","SELECT 1 AS N -- first line\n","SELECT 2 AS N -- second line",(short)0,"A");
      union("UnionUnicode","SELECT 'é🦀' AS N","SELECT 'é🦀'",(short)2,null);
      union("UnionNulls","SELECT NULL AS N","SELECT NULL",(short)2,null);
      db.flush();
      if(db.getQueries().size()!=14)throw new AssertionError("Original query count changed");
    }
    System.out.println("Original UNION metadata created; no SQL executed.");
  }
}
