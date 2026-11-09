import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.impl.query.QueryImpl;
import java.nio.file.*;
import java.util.*;

/** Original physical/key-order disagreements, ties, byte prefixes and NULL keys. */
public class SavedOrderingFixture extends SavedForestFixture {
  static int definition(String name, int family, int type) throws Exception {
    int id = nextId++;
    Map<String,Object> object = new HashMap<>();
    object.put("Id",id); object.put("ParentId",parent); object.put("Name",name);
    object.put("Type",(short)5); object.put("Flags",family); catalog.addRowFromMap(object);
    row(id,(byte)1,null,(short)type,null,null,1);
    row(id,(byte)3,null,(short)0,null,null,1);
    return id;
  }
  static void keyed(int id, byte attribute, String expression, Short flag,
                    String name1, String name2, byte[] order) throws Exception {
    queries.addRowFromMap(new QueryImpl.Row(null,attribute,expression,flag,null,name1,name2,id,order).toTableRow());
  }
  static void columns(String name, byte[] first, byte[] second, boolean nullOther) throws Exception {
    int id = definition(name,0,1);
    if(nullOther) keyed(id,(byte)0,null,null,null,null,new byte[]{1});
    keyed(id,(byte)6,"2",null,"C2",null,first);
    keyed(id,(byte)6,"1",null,"C1",null,second);
  }
  public static void main(String[] args) throws Exception {
    SavedForestFixture.main(args);
    try(Database db = new DatabaseBuilder(Path.of(args[0])).open()) {
      db.setEvaluateExpressions(false); db.setAllowAutoNumberInsert(true);
      catalog = db.getSystemTable("MSysObjects"); queries = db.getSystemTable("MSysQueries");
      int id = definition("OrderProjection",0,1);
      keyed(id,(byte)6,"3",null,"C3",null,new byte[]{(byte)128});
      keyed(id,(byte)6,"2",null,"C2",null,new byte[]{0,0});
      keyed(id,(byte)6,"1",null,"C1",null,new byte[]{0});
      keyed(id,(byte)6,"4",null,"C4",null,new byte[]{(byte)129});
      columns("OrderEmpty",new byte[]{0},new byte[]{},false);
      // The native system index requires unique, non-NULL keys. The comparison
      // recipe creates tie/NULL controls only on explicitly inconsistent copies.
      columns("OrderTies",new byte[]{1},new byte[]{2},false);
      columns("OrderNullColumn",new byte[]{(byte)255},new byte[]{1},false);
      columns("OrderNullOther",new byte[]{(byte)255},new byte[]{1},true);
      id = definition("OrderParameters",0,1);
      row(id,(byte)2,null,(short)4,"Floor",null,2);
      row(id,(byte)2,null,(short)4,"Ceiling",null,1);
      row(id,(byte)5,null,null,"AData",null,1);
      row(id,(byte)6,"[ID]",null,"KeyA",null,1);
      row(id,(byte)8,"[ID]>=Floor AND [ID]<=Ceiling",null,null,null,1);
      row(id,(byte)11,"KeyA",null,"A",null,1);
      for(String name : new String[]{"OrderSort","OrderSources"}) {
        id = definition(name,0,1);
        row(id,(byte)5,null,null,"AData","a",2);
        row(id,(byte)5,null,null,"BData","b",1);
        row(id,(byte)6,"a.ID",null,"KeyA",null,1);
        row(id,(byte)6,"b.ID",null,"KeyB",null,2);
        if(name.equals("OrderSort")) {
          row(id,(byte)11,"KeyA",null,"A",null,2);
          row(id,(byte)11,"KeyB",null,"D",null,1);
        }
      }
      id = definition("OrderJoin",0,1);
      for(int i=0;i<3;i++) {
        String alias=Character.toString('a'+i);
        row(id,(byte)5,null,null,TABLES[i],alias,i+1);
        row(id,(byte)6,alias+".ID",null,"Key"+Character.toString('A'+i),null,i+1);
      }
      row(id,(byte)7,"b.ID=c.ID",(short)2,"b","c",3);
      row(id,(byte)7,"b.ID<>2",(short)2,"b","a",2);
      row(id,(byte)7,"a.ID=b.ID",(short)2,"a","b",1);
      row(id,(byte)11,"KeyA",null,"A",null,1);
      id = definition("OrderCrosstab",0x10,6);
      row(id,(byte)5,null,null,"AData",null,1);
      row(id,(byte)6,"Count(*)",(short)2,"Total",null,2);
      row(id,(byte)6,"0",(short)2,"Bucket",null,1);
      row(id,(byte)6,"ID IN (1,2,4)",(short)1,null,null,4);
      row(id,(byte)6,"Count(*)",(short)0,"Measure",null,3);
      row(id,(byte)9,"0",(short)2,null,null,1);
      id = definition("OrderUnion",0x80,9);
      row(id,(byte)5,"SELECT ID AS KeyA,1 AS KeyB FROM BData",null,null,"X7YZ_____2",1);
      row(id,(byte)5,"SELECT ID AS KeyA,0 AS KeyB FROM AData",null,null,"X7YZ_____1",2);
      row(id,(byte)11,"KeyA",null,"A",null,2);
      row(id,(byte)11,"KeyB",null,"D",null,1);
      db.flush();
      if(db.getQueries().size()!=25) throw new AssertionError("Query count changed");
    }
    System.out.println("Original physical/key-order inputs created; no query executed.");
  }
}
