import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.query.Query;
import com.healthmarketscience.jackcess.impl.query.*;
import java.nio.ByteBuffer;
import java.nio.file.*;
import java.util.*;

/** Original inert native fragment recipe through explicit fresh system-table writes. */
public class SavedQueryFixture {
  static void object(Table catalog,int id,int parent,String name,int flags)throws Exception {
    Map<String,Object> row=new HashMap<>();
    row.put("Id",id);row.put("ParentId",parent);row.put("Name",name);row.put("Type",(short)5);row.put("Flags",flags);
    catalog.addRowFromMap(row);
  }
  static void fragment(Table table,int id,Byte attribute,String expression,Short flag,Integer extra,String name1,String name2,byte[] order)throws Exception {
    table.addRowFromMap(new QueryImpl.Row(null,attribute,expression,flag,extra,name1,name2,id,order).toTableRow());
  }
  static byte[] order(int value){return ByteBuffer.allocate(4).putInt(value).array();}
  public static void main(String[] args)throws Exception {
    if(args.length!=2)throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output must not exist");
    var format=switch(args[1]){case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      db.setEvaluateExpressions(false);db.setAllowAutoNumberInsert(true);
      new TableBuilder("Items").addColumn(new ColumnBuilder("ID",DataType.LONG)).toTable(db);
      Table catalog=db.getSystemTable("MSysObjects"),queries=db.getSystemTable("MSysQueries");
      int parent=0;
      for(Row row:catalog)if("Tables".equals(row.get("Name"))&&Objects.equals(row.get("Type"),(short)3))parent=row.getInt("Id");
      if(parent==0)throw new AssertionError("Missing table/query namespace container");
      int id=Integer.MIN_VALUE+100;
      for(Query.Type type:Query.Type.values()) {
        if(type==Query.Type.UNKNOWN)continue;
        String name=type.name()+"_Stored";
        object(catalog,id,parent,name,type.getObjectFlag());
        // Deliberately nonmonotonic Order bytes: this corpus qualifies stored
        // fragments, without claiming valid reconstructed SQL/action behavior.
        fragment(queries,id,QueryFormat.START_ATTRIBUTE,null,null,null,null,null,order(9));
        fragment(queries,id,QueryFormat.TYPE_ATTRIBUTE,"INERT "+type.name(),type.getValue(),Integer.MIN_VALUE,null,"",order(1));
        fragment(queries,id,QueryFormat.FLAG_ATTRIBUTE,null,(short)1,null,null,null,order(8));
        fragment(queries,id,QueryFormat.TABLE_ATTRIBUTE,null,null,null,"Items","src",order(2));
        fragment(queries,id,QueryFormat.COLUMN_ATTRIBUTE,"[src].[ID]",null,null,"Result ID",null,order(7));
        fragment(queries,id,QueryFormat.WHERE_ATTRIBUTE,"NeverRunMe([src].[ID])",null,null,null,null,order(3));
        fragment(queries,id,QueryFormat.ORDERBY_ATTRIBUTE,"[Result ID]",null,Integer.MAX_VALUE,"D",null,order(6));
        fragment(queries,id,QueryFormat.END_ATTRIBUTE,null,null,null,null,null,order(4));
        id++;
      }
      object(catalog,id,parent,"UnknownFragments",0x80000003);
      fragment(queries,id,QueryFormat.TYPE_ATTRIBUTE,null,Query.Type.SELECT.getValue(),null,null,null,order(1));
      fragment(queries,id,(byte)200,"native é🦀\0\tline\n",Short.MIN_VALUE,Integer.MIN_VALUE,"NULL","",new byte[0]);
      // ObjectId/Attribute/Order form the template's required composite index.
      fragment(queries,id,(byte)201,null,null,null,null,null,order(5));
      object(catalog,id+1,parent,"EmptyDefinition",0);
      db.flush();
      List<Query> loaded=db.getQueries();
      if(loaded.size()!=11)throw new AssertionError("Saved query count changed");
      int count=0;
      for(Query query:loaded)count+=((QueryImpl)query).getRows().size();
      if(count!=75)throw new AssertionError("Stored query fragment count changed");
    }
    System.out.println("Original inert saved-query metadata assertions passed.");
  }
}
