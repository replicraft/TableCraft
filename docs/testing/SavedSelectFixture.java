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
  static void source(int id,String table,String alias)throws Exception {source(id,table,alias,1);}
  static void source(int id,String table,String alias,int ordinal)throws Exception {fragment(id,QueryFormat.TABLE_ATTRIBUTE,null,null,table,alias,ordinal);}
  static void column(int id,String expression,String alias,int ordinal)throws Exception {fragment(id,QueryFormat.COLUMN_ATTRIBUTE,expression,null,alias,null,ordinal);}
  static void end(int id)throws Exception {fragment(id,QueryFormat.END_ATTRIBUTE,null,null,null,null,1);}
  static void parameter(int id,String name,int kind,Integer length,int ordinal)throws Exception {
    queries.addRowFromMap(new QueryImpl.Row(null,QueryFormat.PARAMETER_ATTRIBUTE,null,(short)kind,length,name,null,id,order(ordinal)).toTableRow());
  }
  static void parameters()throws Exception {
    int id=start("AllParameterTypes",0,null);
    int[] kinds={1,2,3,4,5,6,7,8,9,10,11,15};
    String[] names={"pBool","pByte","pShort","pLong","pMoney","pSingle","pDouble","pDate","pBinary","pText","pOLE","pGuid"};
    for(int i=0;i<kinds.length;i++)parameter(id,names[i],kinds[i],kinds[i]==10?17:null,i+1);
    column(id,"[pLong]","Chosen",1);end(id);
    id=start("PromptText",0,null);parameter(id,"Pick label?",10,12,1);column(id,"[Pick label?]",null,1);end(id);
    id=start("DefaultTextLength",0,null);parameter(id,"P",10,0,1);column(id,"[P]",null,1);end(id);
    id=start("UnspecifiedTextLength",0,null);parameter(id,"P",10,null,1);column(id,"[P]",null,1);end(id);
    id=start("MaxTextSyntax",0,null);parameter(id,"P",10,Integer.MAX_VALUE,1);column(id,"[P]",null,1);end(id);
    id=start("OrderedParameterClause",4|16,"3");parameter(id,"pFloor",4,null,1);parameter(id,"pCeiling",4,null,2);
    source(id,"Items",null);column(id,"[ID]",null,1);fragment(id,QueryFormat.WHERE_ATTRIBUTE,"[ID] >= [pFloor] And [ID] <= [pCeiling]",null,null,null,1);
    fragment(id,QueryFormat.ORDERBY_ATTRIBUTE,"[ID]",null,"D",null,1);end(id);
    id=start("ParameterJoin",1,null);parameter(id,"pFloor",4,null,1);source(id,"Items","a",1);source(id,"RightItems","b",2);
    fragment(id,QueryFormat.JOIN_ATTRIBUTE,"[a].[ID] = [b].[ID]",(short)2,"a","b",1);
    fragment(id,QueryFormat.WHERE_ATTRIBUTE,"[a].[ID] >= [pFloor]",null,null,null,1);end(id);
    id=start("PunctuationName",0,null);parameter(id,"Floor's value?",4,null,1);column(id,"[Floor's value?]",null,1);end(id);
  }
  static void joins()throws Exception {
    String[] kinds={"Inner","Left","Right"};
    for(short kind=1;kind<=3;kind++)for(boolean reverse:new boolean[]{false,true}) {
      int id=start("Join"+kinds[kind-1]+(reverse?"Reverse":"Forward"),1,null);
      if(reverse){source(id,"RightItems","b",1);source(id,"Items","a",2);}
      else {source(id,"Items","a",1);source(id,"RightItems","b",2);}
      fragment(id,QueryFormat.JOIN_ATTRIBUTE,"[a].[ID] = [b].[ID]",kind,"A","b",1);end(id);
    }
    int id=start("JoinSelf",1,null);source(id,"Items","a",1);source(id,"Items","b",2);
    fragment(id,QueryFormat.JOIN_ATTRIBUTE,"[a].[ID] = [b].[ID]",(short)2,"a","b",1);end(id);
    id=start("JoinUnicodeTable",1,null);source(id,"Ordres 🦀","a",1);source(id,"Items","b",2);
    fragment(id,QueryFormat.JOIN_ATTRIBUTE,"[a].[ID] = [b].[ID]",(short)1,"a","b",1);end(id);
    id=start("JoinNamesReverse",1,null);source(id,"Items",null,1);source(id,"RightItems",null,2);
    fragment(id,QueryFormat.JOIN_ATTRIBUTE,"[Items].[ID] = [RightItems].[ID]",(short)3,"rightitems","ITEMS",1);end(id);
  }
  static void compoundJoins()throws Exception {
    String[] kinds={"Inner","Left","Right"};
    String[] conditions={"[a].[ID] = [b].[ID]","[a].[ID] > 0","[b].[ID] >= 1"};
    for(short kind=1;kind<=3;kind++)for(int count:new int[]{2,3}) {
      int id=start("Compound"+kinds[kind-1]+count,1,null);
      source(id,"Items","a",1);source(id,"RightItems","b",2);
      for(int i=0;i<count;i++)fragment(id,QueryFormat.JOIN_ATTRIBUTE,conditions[i],kind,"a","b",i+1);
      end(id);
    }
  }
  public static void main(String[] args)throws Exception {
    if(args.length!=2&&!(args.length==3&&Set.of("--joins","--compound-joins","--parameters").contains(args[2])))throw new IllegalArgumentException("Expected fresh path, V2000/V2007/V2016/V2019 and optional --joins/--compound-joins/--parameters");
    boolean compound=args.length==3&&args[2].equals("--compound-joins");
    boolean joined=compound||(args.length==3&&args[2].equals("--joins"));
    boolean parameterized=args.length==3&&args[2].equals("--parameters");
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output must not exist");
    var format=switch(args[1]){case "V2000"->Database.FileFormat.V2000;case "V2007"->Database.FileFormat.V2007;case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      db.setEvaluateExpressions(false);db.setAllowAutoNumberInsert(true);
      new TableBuilder("Items").addColumn(new ColumnBuilder("ID",DataType.LONG)).addColumn(new ColumnBuilder("Label",DataType.TEXT)).toTable(db).addRow(1,"one");
      new TableBuilder("Order Items").addColumn(new ColumnBuilder("Label Name",DataType.TEXT)).toTable(db).addRow("é🦀");
      if(joined||parameterized) {
        new TableBuilder("RightItems").addColumn(new ColumnBuilder("ID",DataType.LONG)).toTable(db).addRow(1);
        new TableBuilder("Ordres 🦀").addColumn(new ColumnBuilder("ID",DataType.LONG)).toTable(db).addRow(1);
      }
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
      if(joined)joins();
      if(compound)compoundJoins();
      if(parameterized)parameters();
      db.flush();
      if(db.getQueries().size()!=(compound?28:joined?22:parameterized?21:13))throw new AssertionError("Saved SELECT count changed");
      for(Query query:db.getQueries())if(query.getType()!=Query.Type.SELECT)throw new AssertionError("Non-SELECT fixture");
    }
    System.out.println("Original local saved SELECT metadata assertions passed; no SQL executed.");
  }
}
