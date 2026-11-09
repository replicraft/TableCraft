import com.healthmarketscience.jackcess.*;
import java.nio.file.*;
import java.util.*;

/** Original derived-source metadata; shares only our original forest recipe. */
public class SavedDerivedFixture extends SavedForestFixture {
  static void derived(String name, String text, String alias, String expression,
                      boolean joined, boolean lateral, boolean parameter) throws Exception {
    int id = nextId++;
    Map<String,Object> object = new HashMap<>();
    object.put("Id",id); object.put("ParentId",parent); object.put("Name",name);
    object.put("Type",(short)5); object.put("Flags",0); catalog.addRowFromMap(object);
    row(id,(byte)1,null,(short)1,null,null,1);
    row(id,(byte)3,null,(short)0,null,null,1);
    if(parameter) row(id,(byte)2,null,(short)4,"Floor",null,1);
    row(id,(byte)5,text,null,null,alias,1);
    row(id,(byte)6,expression,null,"KeyA",null,1);
    row(id,(byte)11,"[KeyA]",null,"A",null,1);
    if(joined) {
      row(id,(byte)5,null,null,"BData","b",2);
      row(id,(byte)6,"[b].[ID]",null,"KeyB",null,2);
      row(id,(byte)7,"[d].[ID]=[b].[ID]",(short)2,"d","b",1);
      row(id,(byte)11,"[KeyB]",null,"A",null,2);
    }
    if(lateral) row(id,(byte)5,null,null,"AData","a",2);
  }
  public static void main(String[] args) throws Exception {
    SavedForestFixture.main(args);
    try(Database db = new DatabaseBuilder(Path.of(args[0])).open()) {
      db.setEvaluateExpressions(false); db.setAllowAutoNumberInsert(true);
      catalog = db.getSystemTable("MSysObjects"); queries = db.getSystemTable("MSysQueries");
      derived("DerivedLocal","SELECT ID FROM AData WHERE ID>1","d","[d].[ID]",false,false,false);
      derived("DerivedUnion","SELECT ID FROM AData WHERE ID=1 UNION ALL SELECT ID FROM BData WHERE ID=3","d","[d].[ID]",false,false,false);
      derived("DerivedNested","SELECT q.ID FROM (SELECT ID FROM AData) AS q WHERE q.ID<4","d","[d].[ID]",false,false,false);
      derived("DerivedAnonymous","SELECT ID FROM AData",null,"[ID]",false,false,false);
      derived("DerivedJoin","SELECT ID FROM AData WHERE ID<3","d","[d].[ID]",true,false,false);
      derived("DerivedAggregate","SELECT Count(*) AS N FROM AData","d","[d].[N]",false,false,false);
      derived("DerivedUdf","SELECT NeverRunMe(ID) AS ID FROM AData","d","[d].[ID]",false,false,false);
      derived("DerivedLateral","SELECT a.ID AS ID","d","[d].[ID]",false,true,false);
      derived("DerivedOwner","SELECT ID FROM AData WITH OWNERACCESS OPTION","d","[d].[ID]",false,false,false);
      derived("DerivedParameter","SELECT ID FROM AData WHERE ID>=Floor","d","[d].[ID]",false,false,true);
      derived("DerivedNamed","SELECT * FROM GraphLeft","d","[d].[KeyA]",false,false,false);
      db.flush();
      if(db.getQueries().size()!=25) throw new AssertionError("Query count changed");
    }
    System.out.println("Original derived sources created; no query executed.");
  }
}
