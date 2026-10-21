import com.healthmarketscience.jackcess.*;
import java.nio.file.*;

/** Original rate-solver metadata; no SQL, formula or VBA executes. */
public class SavedRateFixture extends SavedStatisticsFixture {
  static void output(int id,String expression,String name,int order)throws Exception {
    row(id,(byte)6,expression,null,name,null,order);
  }
  static void scalar(String name,String expression)throws Exception {
    output(definition(name,0,1),expression,"R",1);
  }
  public static void main(String[] args)throws Exception {
    SavedForestFixture.main(args);
    try(Database db=new DatabaseBuilder(Path.of(args[0])).open()) {
      db.setEvaluateExpressions(false);db.setAllowAutoNumberInsert(true);
      catalog=db.getSystemTable("MSysObjects");queries=db.getSystemTable("MSysQueries");
      scalar("RateAll","Rate(12,-100,1000)");
      scalar("RateFromPayment","Rate(12,Pmt(.01,12,1000),1000,,,)");
      int id=definition("RateParams",0,1);row(id,(byte)2,null,(short)4,"GuessBp",null,1);
      output(id,"Rate(12,-100,1000,0,0,[GuessBp]/10000)","R",1);
      id=definition("RateRows",0,1);from(id);output(id,"ID","Period",1);
      output(id,"Rate(ID,Pmt(.01,ID,1000),1000)","R",2);row(id,(byte)11,"Period",null,"A",null,1);
      id=definition("RateGrouped",0,1);from(id);output(id,"ID Mod 2","G",1);output(id,"Avg(Rate(12,-100,1000))","R",2);
      row(id,(byte)9,"ID Mod 2",null,null,null,1);row(id,(byte)11,"G",null,"A",null,1);
      id=definition("RateCrosstab",0x10,6);from(id);
      row(id,(byte)6,"ID Mod 2",(short)2,"G",null,1);row(id,(byte)6,"Avg(Rate(12,Pmt(.01,12,1000),1000))",(short)2,"Total",null,2);
      row(id,(byte)6,"Avg(Rate(12,Pmt(.01,12,1000),1000))",(short)0,"Measure",null,3);row(id,(byte)6,"ID IN (1,2,4)",(short)1,null,null,4);
      row(id,(byte)9,"ID Mod 2",(short)2,null,null,1);row(id,(byte)11,"G",null,"A",null,1);
      id=definition("RateDerived",0,1);row(id,(byte)5,"SELECT Rate(12,-100,1000) AS R",null,null,"d",1);output(id,"d.R","R",1);
      id=definition("RateUnion",0x80,9);row(id,(byte)5,"SELECT Rate(12,-100,1000) AS R",null,null,"X7YZ_____1",1);row(id,(byte)5,"SELECT Rate(1,0,100,-95,0,0) AS R",null,null,"X7YZ_____2",2);
      scalar("RateNoConvergence","Rate(12,-100,1000,,,1e100)");scalar("RateImpossible","Rate(12,100,1000)");
      id=definition("RateLate",0,1);from(id);output(id,"Rate(1,0,100,-95,0,IIf(ID=4,-1,0))","R",1);
      db.flush();if(db.getQueries().size()!=25)throw new AssertionError("Query count changed");
    }
    System.out.println("Original rate query definitions created without execution.");
  }
}
