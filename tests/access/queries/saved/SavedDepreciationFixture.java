import com.healthmarketscience.jackcess.*;
import java.nio.file.*;

/** Original inert depreciation metadata; no SQL or formula is executed. */
public class SavedDepreciationFixture extends SavedStatisticsFixture {
  static void output(int id,String expression,String name,int order)throws Exception {
    row(id,(byte)6,expression,null,name,null,order);
  }
  public static void main(String[] args)throws Exception {
    SavedForestFixture.main(args);
    try(Database db=new DatabaseBuilder(Path.of(args[0])).open()) {
      db.setEvaluateExpressions(false);db.setAllowAutoNumberInsert(true);
      catalog=db.getSystemTable("MSysObjects");queries=db.getSystemTable("MSysQueries");
      int id=definition("DepreciationAll",0,1);
      output(id,"SLN(1200,200,4)","Straight",1);output(id,"SYD(1200,200,4,1)","Weighted",2);output(id,"DDB(1200,200,4,1,)","Declining",3);
      id=definition("DepreciationRows",0,1);from(id);output(id,"ID","Period",1);
      output(id,"SLN(1200,200,4)","Straight",2);output(id,"SYD(1200,200,4,ID)","Weighted",3);output(id,"DDB(1200,200,4,ID)","Declining",4);row(id,(byte)11,"Period",null,"A",null,1);
      id=definition("DepreciationFactor",0,1);from(id);output(id,"DDB(1200,200,4,ID,1.5)","V",1);row(id,(byte)11,"ID",null,"A",null,1);
      id=definition("DepreciationParams",0,1);row(id,(byte)2,null,(short)4,"Per",null,1);output(id,"DDB(1200,200,4,[Per],)","V",1);
      id=definition("DepreciationGrouped",0,1);from(id);output(id,"ID Mod 2","G",1);output(id,"Sum(SYD(1200,200,4,ID))","V",2);
      row(id,(byte)9,"ID Mod 2",null,null,null,1);row(id,(byte)11,"G",null,"A",null,1);
      id=definition("DepreciationCrosstab",0x10,6);from(id);
      row(id,(byte)6,"ID Mod 2",(short)2,"G",null,1);row(id,(byte)6,"Sum(DDB(1200,200,4,ID))",(short)2,"Total",null,2);
      row(id,(byte)6,"Sum(DDB(1200,200,4,ID))",(short)0,"Measure",null,3);row(id,(byte)6,"ID IN (1,2,4)",(short)1,null,null,4);
      row(id,(byte)9,"ID Mod 2",(short)2,null,null,1);row(id,(byte)11,"G",null,"A",null,1);
      id=definition("DepreciationDerived",0,1);row(id,(byte)5,"SELECT SLN(1200,200,ID) AS V FROM AData",null,null,"d",1);
      output(id,"d.V","V",1);row(id,(byte)11,"V",null,"D",null,1);
      id=definition("DepreciationUnion",0x80,9);row(id,(byte)5,"SELECT SLN(1200,200,4) AS V",null,null,"X7YZ_____1",1);row(id,(byte)5,"SELECT DDB(1200,200,4,1) AS V",null,null,"X7YZ_____2",2);
      id=definition("DepreciationInvalid",0,1);from(id);output(id,"SYD(1200,200,4,ID+2)","V",1);
      id=definition("DepreciationNull",0,1);output(id,"DDB(1200,200,4,1,Null)","V",1);
      db.flush();if(db.getQueries().size()!=24)throw new AssertionError("Query count changed");
    }
    System.out.println("Original depreciation query definitions created without execution.");
  }
}
