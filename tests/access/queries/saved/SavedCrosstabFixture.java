import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.impl.query.*;
import java.nio.ByteBuffer;
import java.nio.file.*;
import java.util.*;

/** Original inert crosstab metadata; no SQL, macros, providers or VBA are run. */
public class SavedCrosstabFixture {
  static Table catalog,queries;
  static int parent,nextId=Integer.MIN_VALUE+800;
  static void row(int id,byte attribute,String expression,Short flag,String name1,String name2,int order)throws Exception {
    queries.addRowFromMap(new QueryImpl.Row(null,attribute,expression,flag,null,name1,name2,id,ByteBuffer.allocate(4).putInt(order).array()).toTableRow());
  }
  static int start(String name,boolean cross,short flags,String top)throws Exception {
    int id=nextId++;Map<String,Object> values=new HashMap<>();
    values.put("Id",id);values.put("ParentId",parent);values.put("Name",name);values.put("Type",(short)5);values.put("Flags",cross?16:0);
    catalog.addRowFromMap(values);row(id,(byte)1,null,(short)(cross?6:1),null,null,1);row(id,(byte)3,null,flags,top,null,1);return id;
  }
  static int cross(String name,String source,String qualifier,String pivot,String pivotBody,short flags,String top,String alias,String... aggregate)throws Exception {
    return crossSorted(name,source,qualifier,pivot,pivotBody,flags,top,alias,aggregate.length==0?null:aggregate[0],"[Region]","A");
  }
  static int crossSorted(String name,String source,String qualifier,String pivot,String pivotBody,short flags,String top,String alias,String aggregate,String sort,String direction)throws Exception {
    int id=start(name,true,flags,top);
    row(id,(byte)5,null,null,source,qualifier.isEmpty()?null:qualifier,1);
    String region=qualifier.isEmpty()?"[Region]":"["+qualifier+"].[Region]";
    String amount=qualifier.isEmpty()?"[Amount]":"["+qualifier+"].[Amount]";
    row(id,(byte)6,region,(short)2,"Region",null,1);
    row(id,(byte)6,"Sum("+amount+")",(short)2,"Total",null,2);
    row(id,(byte)6,aggregate==null?"Sum("+amount+")":aggregate,(short)0,alias,null,3);
    row(id,(byte)6,pivot,(short)1,null,null,4);
    row(id,(byte)9,region,(short)2,null,null,1);
    row(id,(byte)9,pivotBody,(short)1,null,null,2);
    row(id,(byte)11,sort,null,direction,null,1);
    return id;
  }
  public static void main(String[] args)throws Exception {
    if(args.length!=2&&!(args.length==3&&args[2].equals("--top")))throw new IllegalArgumentException("Expected fresh path, V2000/V2007/V2016/V2019 and optional --top");
    boolean top=args.length==3;
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output must not exist");
    var format=switch(args[1]){case "V2000"->Database.FileFormat.V2000;case "V2007"->Database.FileFormat.V2007;case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      db.setEvaluateExpressions(false);db.setAllowAutoNumberInsert(true);
      Table sales=new TableBuilder("Sales").addColumn(new ColumnBuilder("Region",DataType.LONG)).addColumn(new ColumnBuilder("Quarter",DataType.LONG)).addColumn(new ColumnBuilder("Amount",DataType.LONG)).toTable(db);
      for(Object[] values:new Object[][]{{1,2,10},{1,1,3},{1,2,7},{2,3,5},{2,1,null},{3,null,8}})sales.addRow(values);
      Table regions=new TableBuilder("Regions").addColumn(new ColumnBuilder("Region",DataType.LONG)).toTable(db);
      for(int region:new int[]{1,2,3})regions.addRow(region);
      catalog=db.getSystemTable("MSysObjects");queries=db.getSystemTable("MSysQueries");
      for(Row value:catalog)if("Tables".equals(value.get("Name"))&&Objects.equals(value.get("Type"),(short)3))parent=value.getInt("Id");
      if(parent==0)throw new AssertionError("Missing query namespace");
      int id=start("LocalSales",false,(short)1,null);row(id,(byte)5,null,null,"Sales",null,1);
      cross("CrossCount","Sales","","[Quarter]","[Quarter]",(short)0,null,"Measure","Count(*)");
      cross("CrossDynamic","Sales","","[Quarter]","[Quarter]",(short)0,null,"Measure");
      cross("CrossFixed","Sales","","[Quarter] IN (3,2,4)","[Quarter]",(short)0,null,"Measure");
      id=cross("CrossHaving","Sales","","[Quarter]","[Quarter]",(short)0,null,"Measure");row(id,(byte)10,"Sum([Amount]) > 10",null,null,null,1);
      id=cross("CrossFiltered","Sales","","[Quarter]","[Quarter]",(short)0,null,"Measure");row(id,(byte)8,"[Quarter] = 2",null,null,null,1);
      id=cross("CrossParameter","Sales","","[Quarter]","[Quarter]",(short)0,null,"Measure");row(id,(byte)2,null,(short)4,"Floor",null,1);row(id,(byte)8,"[Region] >= [Floor]",null,null,null,1);
      String text="IIf([Quarter] = 2,'B','A')";
      cross("CrossText","Sales","",text+" IN ('B','A','C')",text,(short)0,null,"Measure");
      cross("CrossFromSaved","LocalSales","","[Quarter]","[Quarter]",(short)0,null,"Measure");
      id=cross("CrossJoin","Sales","s","[s].[Quarter]","[s].[Quarter]",(short)0,null,"Measure");row(id,(byte)5,null,null,"Regions","r",2);row(id,(byte)7,"[s].[Region]=[r].[Region]",(short)1,"s","r",1);
      cross("CrossOwner","Sales","","[Quarter]","[Quarter]",(short)4,null,"Measure");
      cross("CrossHiddenTop","Sales","","[Quarter]","[Quarter]",(short)16,"1","Measure");
      id=start("CrossUdf",true,(short)0,null);row(id,(byte)5,null,null,"Sales",null,1);
      row(id,(byte)6,"[Region]",(short)2,null,null,1);row(id,(byte)6,"NeverRunMe()",(short)0,null,null,2);row(id,(byte)6,"[Quarter]",(short)1,null,null,3);row(id,(byte)9,"[Region]",(short)2,null,null,1);
      cross("CrossAggregateAlias","Sales","","[Quarter]","[Quarter]",(short)0,null,"Montant é");
      if(top) {
        cross("CrossTopPercent","Sales","","[Quarter]","[Quarter]",(short)48,"34","Measure");
        crossSorted("CrossTopTies","Sales","","[Quarter]","[Quarter]",(short)16,"1","Measure",null,"[Region] Mod 2","D");
        id=crossSorted("CrossTopComposite","Sales","","[Quarter]","[Quarter]",(short)16,"1","Measure",null,"[Region] Mod 2","D");row(id,(byte)11,"[Region]",null,"A",null,2);
        id=cross("CrossTopHaving","Sales","","[Quarter]","[Quarter]",(short)48,"50","Measure");row(id,(byte)10,"Sum([Amount]) < 10",null,null,null,1);
        cross("CrossTopError","Sales","","[Quarter]","[Quarter]",(short)16,"1","Measure","Sum([Amount]/([Region]-2))");
        cross("CrossTopZero","Sales","","[Quarter]","[Quarter]",(short)16,"0","Measure");
      }
      db.flush();
      if(db.getQueries().size()!=(top?20:14))throw new AssertionError("Original query count changed");
    }
    System.out.println("Original crosstab metadata created; no SQL executed.");
  }
}
