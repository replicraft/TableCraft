import com.healthmarketscience.jackcess.*;
import java.nio.file.*;
import java.util.*;

/** Original inert aggregate metadata. No SQL, expression or script executes. */
public class SavedStatisticsFixture extends SavedForestFixture {
  static int definition(String name, int family, int type) throws Exception {
    int id=nextId++; Map<String,Object> object=new HashMap<>();
    object.put("Id",id);object.put("ParentId",parent);object.put("Name",name);
    object.put("Type",(short)5);object.put("Flags",family);catalog.addRowFromMap(object);
    row(id,(byte)1,null,(short)type,null,null,1);
    row(id,(byte)3,null,(short)0,null,null,1);
    return id;
  }
  static void measures(int id,String expression) throws Exception {
    String[] calls={"Var","VarP","StDev","StDevP"};String[] names={"V","VP","S","SP"};
    for(int i=0;i<calls.length;i++)row(id,(byte)6,calls[i]+"("+expression+")",null,names[i],null,i+1);
  }
  static void from(int id) throws Exception { row(id,(byte)5,null,null,"AData",null,1); }
  public static void main(String[] args) throws Exception {
    SavedForestFixture.main(args);
    try(Database db=new DatabaseBuilder(Path.of(args[0])).open()) {
      db.setEvaluateExpressions(false);db.setAllowAutoNumberInsert(true);
      catalog=db.getSystemTable("MSysObjects");queries=db.getSystemTable("MSysQueries");
      int id=definition("StatsAll",0,1);from(id);measures(id,"ID");
      id=definition("StatsGrouped",0,1);from(id);measures(id,"ID");
      row(id,(byte)6,"ID Mod 2",null,"G",null,5);
      row(id,(byte)9,"ID Mod 2",null,null,null,1);
      row(id,(byte)11,"G",null,"A",null,1);
      id=definition("StatsSingleton",0,1);from(id);measures(id,"ID");row(id,(byte)8,"ID=1",null,null,null,1);
      id=definition("StatsEmpty",0,1);from(id);measures(id,"ID");row(id,(byte)8,"ID>99",null,null,null,1);
      id=definition("StatsParams",0,1);from(id);measures(id,"ID+[Shift]");row(id,(byte)2,null,(short)4,"Shift",null,1);
      id=definition("StatsCrosstab",0x10,6);from(id);
      row(id,(byte)6,"0",(short)2,"Bucket",null,1);
      row(id,(byte)6,"Var(ID)",(short)2,"Total",null,2);
      row(id,(byte)6,"StDevP(ID)",(short)0,"Measure",null,3);
      row(id,(byte)6,"ID Mod 2 IN (0,1)",(short)1,null,null,4);
      row(id,(byte)9,"0",(short)2,null,null,1);
      id=definition("StatsDerived",0,1);
      row(id,(byte)5,"SELECT Var(ID) AS V,VarP(ID) AS VP,StDev(ID) AS S,StDevP(ID) AS SP FROM AData",null,null,"d",1);
      String[] names={"V","VP","S","SP"};for(int i=0;i<names.length;i++)row(id,(byte)6,"d."+names[i],null,names[i],null,i+1);
      id=definition("StatsUnion",0x80,9);
      row(id,(byte)5,"SELECT VarP(ID) AS V FROM AData",null,null,"X7YZ_____1",1);
      row(id,(byte)5,"SELECT StDevP(ID) AS V FROM AData",null,null,"X7YZ_____2",2);
      id=definition("StatsWrongType",0,1);from(id);row(id,(byte)6,"Var('text')",null,"V",null,1);
      db.flush();if(db.getQueries().size()!=23)throw new AssertionError("Query count changed");
    }
    System.out.println("Original statistical query definitions created without execution.");
  }
}
