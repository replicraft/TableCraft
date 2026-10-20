import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.impl.TableImpl;
import java.nio.file.Files;
import java.nio.file.Path;

// Original engineering recipe; no copied third-party implementation or fixture.
public class LargeTableFixture {
  private static final int ROWS = 6000;
  public static void main(String[] args) throws Exception {
    if(args.length != 2) throw new IllegalArgumentException("Expected new output path and format");
    Path output=Path.of(args[0]);
    if(Files.exists(output)) throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format=switch(args[1]) {
      case "V2000" -> Database.FileFormat.V2000;
      case "V2007" -> Database.FileFormat.V2007;
      case "V2016" -> Database.FileFormat.V2016;
      case "V2019" -> Database.FileFormat.V2019;
      default -> throw new IllegalArgumentException("Unsupported engineering format");
    };
    try(Database db=new DatabaseBuilder(output).setFileFormat(format).create()) {
      TableBuilder builder=new TableBuilder("LargeRows").addColumn(new ColumnBuilder("ID",DataType.LONG));
      for(int column=0;column<6;column++) {
        builder.addColumn(new ColumnBuilder("P"+column,DataType.TEXT).setLengthInUnits(255).setCompressedUnicode(false));
      }
      Table table=builder.toTable(db);
      for(int id=1;id<=ROWS;id++) {
        Object[] values=new Object[7];values[0]=id;
        for(int column=0;column<6;column++) values[column+1]=String.format(java.util.Locale.ROOT,"%06d:",id)+("p"+column).repeat(124);
        table.addRow(values);
      }
      int owned=((TableImpl)table).getOwnedPagesCursor().getUsageMap().getPageCount();
      if(table.getRowCount()!=ROWS || owned<=4096) throw new IllegalStateException("Fixture did not exceed the materialized map ceiling");
      System.out.println("Rows\t"+ROWS+"\tOwnedPages\t"+owned);
    }
  }
}
