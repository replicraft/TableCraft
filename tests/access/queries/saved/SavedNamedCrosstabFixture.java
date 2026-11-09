import com.healthmarketscience.jackcess.*;
import java.nio.file.*;

/** Original fixed-schema dependencies; getters never execute stored queries. */
public class SavedNamedCrosstabFixture extends SavedCrosstabFixture {
  static int select(String name, String source, String alias, String[] expressions,
                    String[] labels) throws Exception {
    int id = start(name,false,(short)0,null);
    row(id,(byte)5,null,null,source,alias,1);
    for(int i=0;i<expressions.length;i++) row(id,(byte)6,expressions[i],null,labels[i],null,i+1);
    row(id,(byte)11,labels[0],null,"A",null,1);
    return id;
  }
  public static void main(String[] args) throws Exception {
    SavedCrosstabFixture.main(args);
    try(Database db = new DatabaseBuilder(Path.of(args[0])).open()) {
      db.setEvaluateExpressions(false); db.setAllowAutoNumberInsert(true);
      catalog = db.getSystemTable("MSysObjects"); queries = db.getSystemTable("MSysQueries");
      int id = cross("CrossFixedParameter","Sales","","Quarter IN (1,2,3)","Quarter",(short)0,null,"Measure");
      row(id,(byte)2,null,(short)4,"Floor",null,1);
      row(id,(byte)8,"Region>=Floor",null,null,null,1);
      String text = "IIf(Quarter=2,'Sales.Q2','Sales!Q1')";
      cross("CrossFixedText","Sales","",text+" IN ('Sales.Q2','Sales!Q1')",text,(short)0,null,"Measure");
      cross("CrossFixedTop","Sales","","Quarter IN (3,2,4)","Quarter",(short)16,"1","Measure");
      crossSorted("CrossFixedTopTies","Sales","","Quarter IN (3,2,4)","Quarter",(short)16,"1","Measure",null,"Region Mod 2","D");
      cross("CrossFixedError","Sales","","Quarter IN (1,2,3)","Quarter",(short)0,null,"Measure","Sum(Amount/(Region-2))");
      cross("CrossFixedCycle","NamedCycle","","Quarter IN (1,2,3)","Quarter",(short)0,null,"Measure");
      select("NamedFixed","CrossFixed",null,new String[]{"Region","[3]","[2]","[4]"},new String[]{"Region","Three","Two","Four"});
      id = select("NamedProjection","CrossFixed",null,new String[]{"Region"},new String[]{"Region"});
      row(id,(byte)8,"[2]>10",null,null,null,1);
      id = select("NamedJoin","CrossFixed","c",new String[]{"c.Region","c.[2]"},new String[]{"Region","Two"});
      row(id,(byte)5,null,null,"Regions","r",2);
      row(id,(byte)7,"c.Region=r.Region",(short)1,"c","r",1);
      row(id,(byte)8,"r.Region=1",null,null,null,1);
      id = start("NamedUnion",false,(short)0,null);
      // This definition is UNION; preserve its native family/type explicitly.
      for(Row object:catalog) if(object.getInt("Id")==id) {object.put("Flags",128);catalog.updateRow(object);break;}
      for(Row part:queries) if(part.getInt("ObjectId")==id && part.getByte("Attribute")==1) {part.put("Flag",(short)9);queries.updateRow(part);break;}
      row(id,(byte)5,"SELECT Region FROM CrossFixed WHERE Region=1",null,null,"X7YZ_____1",1);
      row(id,(byte)5,"SELECT Region FROM CrossFixed WHERE Region=2",null,null,"X7YZ_____2",2);
      row(id,(byte)11,"Region",null,"A",null,1);
      select("NamedCorrelated","Regions",null,new String[]{"Region","(SELECT [2] FROM CrossFixed AS c WHERE c.Region=Regions.Region)"},new String[]{"Region","Two"});
      id = start("NamedNested",false,(short)0,null);
      row(id,(byte)5,"SELECT * FROM CrossFixed",null,null,"d",1);
      row(id,(byte)6,"d.Region",null,"Region",null,1);row(id,(byte)6,"d.[2]",null,"Two",null,2);
      row(id,(byte)8,"d.Region=1",null,null,null,1);row(id,(byte)11,"Region",null,"A",null,1);
      select("NamedParameter","CrossFixedParameter",null,new String[]{"Region","[3]"},new String[]{"Region","Known"});
      select("NamedText","CrossFixedText","t",new String[]{"t.Region","t.[Sales.Q2]"},new String[]{"Region","Two"});
      select("NamedTop","CrossFixedTop",null,new String[]{"Region","Total","[3]","[2]","[4]"},new String[]{"Region","Total","Three","Two","Four"});
      select("NamedTopTies","CrossFixedTopTies",null,new String[]{"Region"},new String[]{"Region"});
      id = select("NamedError","CrossFixedError",null,new String[]{"Region"},new String[]{"Region"});
      row(id,(byte)8,"Region=1",null,null,null,1);
      select("NamedCycle","CrossFixedCycle",null,new String[]{"Region"},new String[]{"Region"});
      id = start("NamedCrosstab",true,(short)0,null);
      row(id,(byte)5,null,null,"CrossFixed",null,1);
      row(id,(byte)6,"0",(short)2,"Bucket",null,1);
      row(id,(byte)6,"Count(*)",(short)0,"Measure",null,2);
      row(id,(byte)6,"Region IN (1,2,4)",(short)1,null,null,3);
      row(id,(byte)9,"0",(short)2,null,null,1);
      db.flush();
      if(db.getQueries().size()!=33) throw new AssertionError("Query count changed");
    }
    System.out.println("Original named fixed crosstab inputs created; no query executed.");
  }
}
