import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.query.Query;
import com.healthmarketscience.jackcess.impl.query.*;
import java.nio.ByteBuffer;
import java.nio.file.*;
import java.util.*;

/** Original local SELECT fragments; metadata creation does not execute queries. */
public class SavedSelectFixture {
  static Table catalog, queries;
  static int parent, nextId=Integer.MIN_VALUE+200;
  static byte[] order(int value){return ByteBuffer.allocate(4).putInt(value).array();}
  static void fragment(int id,byte attribute,String expression,Short flag,String name1,String name2,int ordinal)throws Exception {
    queries.addRowFromMap(new QueryImpl.Row(null,attribute,expression,flag,null,name1,name2,id,order(ordinal)).toTableRow());
  }
  static int start(String name,short flags,String top,boolean type)throws Exception {
    int id=nextId++;
    Map<String,Object> row=new HashMap<>();
    row.put("Id",id);row.put("ParentId",parent);row.put("Name",name);row.put("Type",(short)5);row.put("Flags",0);
    catalog.addRowFromMap(row);
    fragment(id,QueryFormat.START_ATTRIBUTE,null,null,null,null,1);
    if(type)fragment(id,QueryFormat.TYPE_ATTRIBUTE,null,Query.Type.SELECT.getValue(),null,null,1);
    fragment(id,QueryFormat.FLAG_ATTRIBUTE,null,flags,top,null,1);
    return id;
  }
  static int start(String name,int flags,String top)throws Exception{return start(name,(short)flags,top,true);}
  static void source(int id,String table,String alias)throws Exception {fragment(id,QueryFormat.TABLE_ATTRIBUTE,null,null,table,alias,1);}
  static void column(int id,String expression,String alias,int ordinal)throws Exception {fragment(id,QueryFormat.COLUMN_ATTRIBUTE,expression,null,alias,null,ordinal);}
  static void end(int id)throws Exception {fragment(id,QueryFormat.END_ATTRIBUTE,null,null,null,null,1);}
  public static void main(String[] args)throws Exception {
    if(args.length!=2)throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output must not exist");
    var format=switch(args[1]){case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      db.setEvaluateExpressions(false);db.setAllowAutoNumberInsert(true);
      new TableBuilder("Items").addColumn(new ColumnBuilder("ID",DataType.LONG)).addColumn(new ColumnBuilder("Label",DataType.TEXT)).toTable(db).addRow(1,"one");
      new TableBuilder("Order Items").addColumn(new ColumnBuilder("Label Name",DataType.TEXT)).toTable(db).addRow("é🦀");
      catalog=db.getSystemTable("MSysObjects");queries=db.getSystemTable("MSysQueries");
      for(Row row:catalog)if("Tables".equals(row.get("Name"))&&Objects.equals(row.get("Type"),(short)3))parent=row.getInt("Id");
      if(parent==0)throw new AssertionError("Missing query namespace container");
      int id=start("Constants",0,null);column(id,"42","Answer",1);end(id);
      id=start("AllItems",1,null);source(id,"Items",null);end(id);
      id=start("Columns",0,null);source(id,"Items","src");column(id,"[src].[ID]","Result ID",1);column(id,"[src].[Label]","Result Label",2);end(id);
      id=start("DistinctLabels",2,null);source(id,"Items",null);column(id,"[Label]",null,1);fragment(id,QueryFormat.WHERE_ATTRIBUTE,"[Label] Like '*'",null,null,null,1);fragment(id,QueryFormat.ORDERBY_ATTRIBUTE,"[Label]",null,"D",null,1);end(id);
      id=start("TopPercent",16|32,"50");source(id,"Items",null);column(id,"[ID]",null,1);end(id);
      id=start("DistinctRowOwner",1|8|4,null);source(id,"Items","src");end(id);
      id=start("Grouped",0,null);source(id,"Items",null);column(id,"[ID]",null,1);column(id,"[Label]",null,2);column(id,"Count(*)","Total",3);
      fragment(id,QueryFormat.GROUPBY_ATTRIBUTE,"[ID]",null,null,null,1);fragment(id,QueryFormat.GROUPBY_ATTRIBUTE,"[Label]",null,null,null,2);
      fragment(id,QueryFormat.HAVING_ATTRIBUTE,"Count(*) > 0",null,null,null,1);
      fragment(id,QueryFormat.ORDERBY_ATTRIBUTE,"[Total]",null,"D",null,1);fragment(id,QueryFormat.ORDERBY_ATTRIBUTE,"[ID]",null,"A",null,2);end(id);
      id=start("TopZero",16,"0");column(id,"1",null,1);end(id);
      id=start("ExplicitStar",0,null);source(id,"Items",null);column(id,"*",null,1);end(id);
      id=start("UnicodeNames",0,null);source(id,"Order Items","src");column(id,"[src].[Label Name]","Résultat 🦀",1);end(id);
      id=start("FunctionInert",0,null);source(id,"Items",null);column(id,"NeverRunMe([ID])","Probe",1);end(id);
      id=start("SourceIsQuery",1,null);source(id,"AllItems",null);end(id);
      id=start("OmittedType",(short)9,null,false);source(id,"Items",null);end(id);
      db.flush();
      if(db.getQueries().size()!=13)throw new AssertionError("Saved SELECT count changed");
      for(Query query:db.getQueries())if(query.getType()!=Query.Type.SELECT)throw new AssertionError("Non-SELECT fixture");
    }
    System.out.println("Original local saved SELECT metadata assertions passed; no SQL executed.");
  }
}
