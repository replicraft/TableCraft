import com.healthmarketscience.jackcess.*;
import java.nio.file.*;
import java.util.*;

/** Original literal prompt metadata; no host control, SQL or function executes. */
public class SavedPromptFixture extends SavedForestFixture {
  static int definition(String name, int family, int type, String prompt) throws Exception {
    int id = nextId++;
    Map<String,Object> object = new HashMap<>();
    object.put("Id",id); object.put("ParentId",parent); object.put("Name",name);
    object.put("Type",(short)5); object.put("Flags",family); catalog.addRowFromMap(object);
    row(id,(byte)1,null,(short)type,null,null,1);
    row(id,(byte)3,null,(short)0,null,null,1);
    row(id,(byte)2,null,(short)4,prompt,null,1);
    return id;
  }
  static void select(String name, String stored, String literal) throws Exception {
    int id = definition(name,0,1,stored);
    row(id,(byte)5,null,null,"AData",null,1);
    row(id,(byte)6,"ID",null,"KeyA",null,1);
    row(id,(byte)8,"ID>=["+literal+"]",null,null,null,1);
    row(id,(byte)11,"KeyA",null,"A",null,1);
  }
  public static void main(String[] args) throws Exception {
    SavedForestFixture.main(args);
    try(Database db = new DatabaseBuilder(Path.of(args[0])).open()) {
      db.setEvaluateExpressions(false); db.setAllowAutoNumberInsert(true);
      catalog = db.getSystemTable("MSysObjects"); queries = db.getSystemTable("MSysQueries");
      select("PromptDot","Enter No. of items","Enter No. of items");
      select("PromptDotQuoted","[Enter No. of items]","Enter No. of items");
      select("PromptShortDot","P.Q","P.Q");
      select("PromptBang","Floor!","Floor!");
      select("PromptBangQuoted","[Ask!Now]","Ask!Now");
      select("PromptBangSpaces","Ask! now","Ask! now");
      select("PromptGrave","Rate `value`","Rate `value`");
      select("PromptEquals","Floor>= minimum","Floor>= minimum");
      select("PromptLooksLikeCode","NeverRunMe()","NeverRunMe()");
      select("PromptAccent", "é. nombre", "é. nombre");
      select("PromptDecomposed", "e\u0301. nombre", "e\u0301. nombre");
      select("PromptCyrillic", "Введите число", "Введите число");
      select("PromptCJK", "最低値", "最低値");
      select("PromptCJKQuoted", "[最低値]", "最低値");
      select("PromptEmoji", "🦀>= minimum", "🦀>= minimum");
      select("PromptUnicodeCase", "É. nombre", "É. nombre");
      int unicode = definition("PromptUnicodeNested",0,1,"最低値.");
      row(unicode,(byte)5,"SELECT ID FROM AData WHERE ID>=[最低値.]",null,null,"d",1);
      row(unicode,(byte)6,"d.ID",null,"KeyA",null,1);
      row(unicode,(byte)11,"KeyA",null,"A",null,1);
      unicode = definition("PromptUnicodeUnion",0x80,9,"Введите число");
      row(unicode,(byte)5,"SELECT ID AS KeyA FROM AData WHERE ID>=[Введите число]",null,null,"X7YZ_____1",1);
      row(unicode,(byte)5,"SELECT ID AS KeyA FROM BData WHERE ID<=[Введите число]",null,null,"X7YZ_____2",2);
      row(unicode,(byte)11,"KeyA",null,"A",null,1);
      unicode = definition("PromptUnicodeCrosstab",0x10,6,"🦀>= minimum");
      row(unicode,(byte)5,null,null,"AData",null,1);
      row(unicode,(byte)6,"0",(short)2,"Bucket",null,1);
      row(unicode,(byte)6,"Count(*)",(short)2,"Total",null,2);
      row(unicode,(byte)6,"Count(*)",(short)0,"Measure",null,3);
      row(unicode,(byte)6,"ID IN (1,2,4)",(short)1,null,null,4);
      row(unicode,(byte)8,"ID>=[🦀>= minimum]",null,null,null,1);
      row(unicode,(byte)9,"0",(short)2,null,null,1);
      int id = definition("PromptNested",0,1,"Floor.");
      row(id,(byte)5,"SELECT ID FROM AData WHERE ID>=[Floor.]",null,null,"d",1);
      row(id,(byte)6,"d.ID",null,"KeyA",null,1);
      row(id,(byte)11,"KeyA",null,"A",null,1);
      id = definition("PromptUnion",0x80,9,"P.Q");
      row(id,(byte)5,"SELECT ID AS KeyA FROM AData WHERE ID>=[P.Q]",null,null,"X7YZ_____1",1);
      row(id,(byte)5,"SELECT ID AS KeyA FROM BData WHERE ID<=[P.Q]",null,null,"X7YZ_____2",2);
      row(id,(byte)11,"KeyA",null,"A",null,1);
      id = definition("PromptCrosstab",0x10,6,"Floor.");
      row(id,(byte)5,null,null,"AData",null,1);
      row(id,(byte)6,"0",(short)2,"Bucket",null,1);
      row(id,(byte)6,"Count(*)",(short)2,"Total",null,2);
      row(id,(byte)6,"Count(*)",(short)0,"Measure",null,3);
      row(id,(byte)6,"ID IN (1,2,4)",(short)1,null,null,4);
      row(id,(byte)8,"ID>=[Floor.]",null,null,null,1);
      row(id,(byte)9,"0",(short)2,null,null,1);
      id = definition("PromptControl",0,1,"Forms!F!C");
      row(id,(byte)6,"[Forms]![F]![C]",null,"KeyA",null,1);
      db.flush();
      if(db.getQueries().size()!=37) throw new AssertionError("Query count changed");
    }
    System.out.println("Original literal prompt inputs created; no query executed.");
  }
}
