import com.healthmarketscience.jackcess.*;
import java.nio.file.Path;
import java.nio.charset.StandardCharsets;
import java.util.HexFormat;

// Original physical-cursor controls. No expression evaluation or native Access.
public class TextRangeOracle {
  record Case(String name, String mode, String start, String end, boolean excludeStart, boolean excludeEnd) {}
  public static void main(String[] args) throws Exception {
    try(Database db = new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
      db.setEvaluateExpressions(false);
      Table table = db.getTable(args.length > 2 ? args[2] : "TextIndexed");
      Index index = table.getIndex(args[1]);
      boolean descending = args[1].endsWith("Desc");
      String first = descending ? "é" : "a", last = descending ? "a" : "é";
      Case[] cases = {
        new Case("all", "all", null, null, false, false),
        new Case("null", "null", null, null, false, false),
        new Case("open", "range", null, null, false, false),
        new Case("case", "range", "A  ", "a", false, false),
        new Case("empty", "range", "a", "A  ", true, false),
        new Case("accent", "range", "é", "e\u0301", false, false),
        new Case("interval", "range", first, last, false, false),
        new Case("exclusive", "range", first, last, true, true),
        new Case("start", "range", "a", null, false, false),
        new Case("end", "range", null, "é", false, true),
        new Case("reversed", "range", last, first, false, false),
        new Case("ignored", "range", "", "", false, false),
        new Case("long", "range", "a".repeat(255), "a".repeat(255), false, false),
        new Case("supplementary", "range", "😀".repeat(127) + "x", "😀".repeat(127) + "x", false, false)
      };
      for(Case test : cases) {
        CursorBuilder builder = table.newCursor().setIndex(index);
        if(test.mode().equals("null")) builder.setStartEntry((Object)null).setEndEntry((Object)null);
        if(test.start() != null) builder.setStartEntry(test.start()).setStartRowInclusive(!test.excludeStart());
        if(test.end() != null) builder.setEndEntry(test.end()).setEndRowInclusive(!test.excludeEnd());
        Cursor cursor = builder.toIndexCursor();
        Row row;
        while((row = cursor.getNextRow()) != null) {
          String value = (String)row.get("Label");
          if(test.mode().equals("range") && value == null) continue;
          String tag = value == null ? "N" : "T" + HexFormat.of().formatHex(value.getBytes(StandardCharsets.UTF_8));
          System.out.println(test.name() + "\tI32:" + row.get("ID") + "\t" + tag);
        }
      }
    }
  }
}
