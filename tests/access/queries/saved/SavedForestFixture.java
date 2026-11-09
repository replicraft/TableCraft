import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.impl.query.*;
import java.nio.ByteBuffer;
import java.nio.file.*;
import java.util.*;

/** Original inert join-forest metadata. No SQL or application content executes. */
public class SavedForestFixture {
  static Table catalog,queries;static int parent,nextId=Integer.MIN_VALUE+1000;
  static final String[] TABLES={"AData","BData","CData","DData"};
  record Edge(short kind,String left,String right,String expression) {}
  static Edge e(int kind,String left,String right){return new Edge((short)kind,left,right,"["+left+"].[ID]=["+right+"].[ID]");}
  static Edge p(int kind,String left,String right,String expression){return new Edge((short)kind,left,right,expression);}
  static void row(int id,byte attribute,String expression,Short flag,String name1,String name2,int order)throws Exception {
    queries.addRowFromMap(new QueryImpl.Row(null,attribute,expression,flag,null,name1,name2,id,ByteBuffer.allocate(4).putInt(order).array()).toTableRow());
  }
  static void query(String name,int sources,Edge...edges)throws Exception {
    int id=nextId++;Map<String,Object> object=new HashMap<>();object.put("Id",id);object.put("ParentId",parent);object.put("Name",name);object.put("Type",(short)5);object.put("Flags",0);catalog.addRowFromMap(object);
    row(id,(byte)1,null,(short)1,null,null,1);row(id,(byte)3,null,(short)0,null,null,1);
    for(int i=0;i<sources;i++) {
      String alias=Character.toString('a'+i),label="Key"+Character.toString('A'+i);
      row(id,(byte)5,null,null,TABLES[i],alias,i+1);
      row(id,(byte)6,"["+alias+"].[ID]",null,label,null,i+1);
      row(id,(byte)11,"["+label+"]",null,"A",null,i+1);
    }
    for(int i=0;i<edges.length;i++){Edge edge=edges[i];row(id,(byte)7,edge.expression(),edge.kind(),edge.left(),edge.right(),i+1);}
  }
  public static void main(String[] args)throws Exception {
    if(args.length!=2)throw new IllegalArgumentException("Expected fresh path and V2000/V2007/V2016/V2019");
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output must not exist");
    var format=switch(args[1]){case "V2000"->Database.FileFormat.V2000;case "V2007"->Database.FileFormat.V2007;case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      db.setEvaluateExpressions(false);db.setAllowAutoNumberInsert(true);
      int[][] inputs={{1,2,4},{2,3,4},{1,2,3},{2,4,5}};
      for(int i=0;i<TABLES.length;i++) {Table table=new TableBuilder(TABLES[i]).addColumn(new ColumnBuilder("ID",DataType.LONG)).toTable(db);for(int value:inputs[i])table.addRow(value);}
      catalog=db.getSystemTable("MSysObjects");queries=db.getSystemTable("MSysQueries");
      for(Row object:catalog)if("Tables".equals(object.get("Name"))&&Objects.equals(object.get("Type"),(short)3))parent=object.getInt("Id");if(parent==0)throw new AssertionError("Missing namespace");
      query("GraphInner",3,e(1,"a","b"),e(1,"b","c"));
      query("GraphLeft",3,e(2,"a","b"),e(2,"b","c"));
      query("GraphRight",3,e(3,"a","b"),e(3,"b","c"));
      query("GraphReverse",3,e(2,"c","b"),e(3,"a","b"));
      query("GraphMixed",3,e(2,"a","b"),e(1,"a","c"));
      query("GraphInnerPredicate",3,e(2,"a","b"),e(2,"b","c"),p(2,"b","a","[b].[ID]<>2"));
      query("GraphAncestorPredicate",3,e(2,"a","b"),e(1,"a","c"),p(1,"a","b","[a].[ID]<>2"));
      query("GraphFour",4,e(2,"a","b"),e(3,"c","d"),e(1,"b","c"));
      query("GraphDisconnected",4,e(2,"a","b"));
      query("GraphComma",3);query("GraphPairComma",2);
      query("GraphPairReverse",2,e(2,"a","b"),p(2,"b","a","[b].[ID]<>2"));
      query("GraphDeepInner",3,e(1,"a","b"),e(1,"b","c"),e(1,"a","c"));
      query("GraphUdf",3,p(1,"a","b","NeverRunMe([a].[ID])"),e(1,"b","c"));
      db.flush();if(db.getQueries().size()!=14)throw new AssertionError("Query count changed");
    }
    System.out.println("Original join forests created; no SQL executed.");
  }
}
