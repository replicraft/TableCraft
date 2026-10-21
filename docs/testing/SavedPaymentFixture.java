import com.healthmarketscience.jackcess.*;
import java.nio.file.*;

/** Original period/payment metadata, without SQL or formula execution. */
public class SavedPaymentFixture extends SavedFinancialFixture {
  static void components(String name,int timing)throws Exception {
    int id=definition(name,0,1);from(id);
    output(id,"ID","Period",1);
    output(id,"IPmt(.01,ID,4,1000,,"+timing+")","Interest",2);
    output(id,"PPmt(.01,ID,4,1000,,"+timing+")","Principal",3);
    output(id,"Pmt(.01,4,1000,,"+timing+")","Payment",4);
    output(id,"NPer(.01,Pmt(.01,4,1000,,"+timing+"),1000,,"+timing+")","Periods",5);
    row(id,(byte)11,"Period",null,"A",null,1);
  }
  public static void main(String[] args)throws Exception {
    SavedFinancialFixture.main(args);
    try(Database db=new DatabaseBuilder(Path.of(args[0])).open()) {
      db.setEvaluateExpressions(false);db.setAllowAutoNumberInsert(true);
      catalog=db.getSystemTable("MSysObjects");queries=db.getSystemTable("MSysQueries");
      components("PeriodEnd",0);components("PeriodBegin",1);
      int id=definition("PeriodParams",0,1);
      row(id,(byte)2,null,(short)4,"Per",null,1);
      output(id,"IPmt(.01,[Per],4,1000,,1)","Interest",1);
      output(id,"PPmt(.01,[Per],4,1000,,1)","Principal",2);
      id=definition("PeriodGrouped",0,1);from(id);
      output(id,"ID Mod 2","G",1);output(id,"Sum(IPmt(.01,ID,4,1000))","Interest",2);
      output(id,"Sum(PPmt(.01,ID,4,1000))","Principal",3);
      row(id,(byte)9,"ID Mod 2",null,null,null,1);row(id,(byte)11,"G",null,"A",null,1);
      id=definition("PeriodCrosstab",0x10,6);from(id);
      row(id,(byte)6,"ID Mod 2",(short)2,"G",null,1);
      row(id,(byte)6,"Sum(IPmt(.01,ID,4,1000))",(short)2,"Total",null,2);
      row(id,(byte)6,"Sum(IPmt(.01,ID,4,1000))",(short)0,"Measure",null,3);
      row(id,(byte)6,"ID IN (1,2,4)",(short)1,null,null,4);
      row(id,(byte)9,"ID Mod 2",(short)2,null,null,1);row(id,(byte)11,"G",null,"A",null,1);
      id=definition("PeriodInvalid",0,1);from(id);output(id,"PPmt(.01,ID-1,4,1000)","V",1);
      id=definition("PeriodImpossible",0,1);output(id,"NPer(.01,-1,1000)","V",1);
      id=definition("PeriodNull",0,1);output(id,"PPmt(.01,1,4,1000,Null)","V",1);
      id=definition("PeriodDerived",0,1);
      row(id,(byte)5,"SELECT IPmt(.01,ID,4,1000) AS I FROM AData",null,null,"d",1);
      output(id,"d.I","Interest",1);row(id,(byte)11,"Interest",null,"A",null,1);
      id=definition("PeriodUnion",0x80,9);
      row(id,(byte)5,"SELECT IPmt(.01,1,4,1000) AS V",null,null,"X7YZ_____1",1);
      row(id,(byte)5,"SELECT IPmt(.01,4,4,1000) AS V",null,null,"X7YZ_____2",2);
      db.flush();if(db.getQueries().size()!=42)throw new AssertionError("Query count changed");
    }
    System.out.println("Original period/payment query definitions created without execution.");
  }
}
