import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import java.nio.file.*;

// Original inert mixed-policy inputs; Jackcess, not Microsoft Access, authors them.
public class NativePolicyFixture {
  static ColumnBuilder text(String name,ColumnImpl.SortOrder order) {
    ColumnBuilder column=new ColumnBuilder(name,DataType.TEXT).setLengthInUnits(16);
    column.setTextSortOrder(order);return column;
  }
  public static void main(String[] args) throws Exception {
    if(args.length!=2 || Files.exists(Path.of(args[0]))) throw new IllegalArgumentException("Expected fresh path and format");
    try(Database db=new DatabaseBuilder(Path.of(args[0])).setFileFormat(Database.FileFormat.valueOf(args[1])).create()) {
      db.setEvaluateExpressions(false);
      Table table=new TableBuilder("PolicyProbe")
        .addColumn(new ColumnBuilder("ID",DataType.LONG))
        .addColumn(text("LegacyText",ColumnImpl.GENERAL_LEGACY_SORT_ORDER))
        .addColumn(text("ModernText",ColumnImpl.GENERAL_SORT_ORDER))
        .addIndex(new IndexBuilder("PrimaryKey").addColumns("ID").setPrimaryKey())
        .addIndex(new IndexBuilder("ModernIndex").addColumns("ModernText"))
        .toTable(db);
      String[][] values={{"\u0080","\u0080"},{"\u0301","\u0301"},{"a","A"},{"ss","\u00df"},{null,null},{"a ","a "}};
      for(int i=0;i<values.length;i++) table.addRow(i+1,values[i][0],values[i][1]);
    }
  }
}
