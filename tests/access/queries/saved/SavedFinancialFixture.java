import com.healthmarketscience.jackcess.*;
import java.nio.file.*;

/** Original inert annuity query metadata. No formula or SQL executes. */
public class SavedFinancialFixture extends SavedStatisticsFixture {
  static void output(int id,String expression,String name,int order)throws Exception {
    row(id,(byte)6,expression,null,name,null,order);
  }
  public static void main(String[] args)throws Exception {
    SavedStatisticsFixture.main(args);
    try(Database db=new DatabaseBuilder(Path.of(args[0])).open()) {
      db.setEvaluateExpressions(false);db.setAllowAutoNumberInsert(true);
      catalog=db.getSystemTable("MSysObjects");queries=db.getSystemTable("MSysQueries");
      int id=definition("FinanceAll",0,1);
      output(id,"Pmt(.01,12,1000)","Payment",1);output(id,"PV(.01,12,-100)","Present",2);output(id,"FV(.01,12,-100)","Future",3);
      id=definition("FinanceParams",0,1);
      row(id,(byte)2,null,(short)4,"BasisPoints",null,1);row(id,(byte)2,null,(short)4,"Periods",null,2);
      output(id,"Pmt(([BasisPoints]/10000),[Periods],1000,,1)","Payment",1);
      output(id,"PV(([BasisPoints]/10000),[Periods],-100,,1)","Present",2);output(id,"FV(([BasisPoints]/10000),[Periods],-100,,1)","Future",3);
      id=definition("FinanceGrouped",0,1);from(id);
      output(id,"ID Mod 2","G",1);output(id,"Sum(FV(0,ID,-10))","V",2);
      row(id,(byte)9,"ID Mod 2",null,null,null,1);row(id,(byte)11,"G",null,"A",null,1);
      id=definition("FinanceDerived",0,1);
      row(id,(byte)5,"SELECT Pmt(0,ID,120) AS V FROM AData",null,null,"d",1);
      output(id,"d.V","V",1);row(id,(byte)11,"V",null,"A",null,1);
      id=definition("FinanceUnion",0x80,9);
      row(id,(byte)5,"SELECT Pmt(0,10,100) AS V",null,null,"X7YZ_____1",1);
      row(id,(byte)5,"SELECT PV(0,10,-100) AS V",null,null,"X7YZ_____2",2);
      id=definition("FinanceCrosstab",0x10,6);from(id);
      row(id,(byte)6,"ID Mod 2",(short)2,"G",null,1);
      row(id,(byte)6,"Sum(FV(0,ID,-10))",(short)2,"Total",null,2);
      row(id,(byte)6,"Sum(FV(0,ID,-10))",(short)0,"Measure",null,3);
      row(id,(byte)6,"ID IN (1,2,4)",(short)1,null,null,4);
      row(id,(byte)9,"ID Mod 2",(short)2,null,null,1);row(id,(byte)11,"G",null,"A",null,1);
      id=definition("FinanceLateError",0,1);from(id);output(id,"Pmt(0,ID-2,120)","V",1);
      id=definition("FinanceWrongType",0,1);output(id,"FV(0,10,'text')","V",1);
      id=definition("FinanceNull",0,1);output(id,"Pmt(Null,10,1000)","V",1);
      db.flush();if(db.getQueries().size()!=32)throw new AssertionError("Query count changed");
    }
    System.out.println("Original financial query definitions created without execution.");
  }
}
