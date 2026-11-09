import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.complex.*;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import com.healthmarketscience.jackcess.impl.complex.*;
import java.nio.*;
import java.nio.file.Path;
import java.util.*;

/** Original stored-order mixed collection oracle; expressions and lookups stay inert. */
public class CollectionPageOracle {
  static String scalar(Object value, DataType type) {
    return MultivalueOracle.encode(value, type == DataType.MEMO ? DataType.TEXT : type);
  }
  static String date(Object value, ColumnImpl column) throws Exception {
    if(value == null) return "N";
    var bytes = column.write(value, 0, ByteOrder.LITTLE_ENDIAN).duplicate();
    if(bytes.remaining() != Long.BYTES) throw new IllegalArgumentException("Expected DATE width");
    return String.format("Y64:%016x", bytes.order(ByteOrder.LITTLE_ENDIAN).getLong());
  }
  public static void main(String[] args) throws Exception {
    if(args.length != 2) throw new IllegalArgumentException("Expected file and table");
    try(Database db = new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
      db.setEvaluateExpressions(false); db.setDateTimeType(DateTimeType.DATE); db.setTimeZone(TimeZone.getTimeZone("UTC"));
      Table table = db.getTable(args[1]);
      for(Column column : table.getColumns()) {
        var info = column.getComplexInfo();
        String kind = info == null ? (column.isCalculated() ? "C" : "S") : switch(info.getType()) {
          case MULTI_VALUE -> "M";
          case ATTACHMENT -> "A";
          case VERSION_HISTORY -> "H:" + ((VersionHistoryColumnInfoImpl)info).getValueColumn().getName();
          default -> throw new IllegalArgumentException("Unqualified collection");
        };
        System.out.println("H\t" + column.getName() + "\t" + kind);
      }
      int ordinal = 0;
      for(Row parent : table) {
        for(Column column : table.getColumns()) {
          StringBuilder row = new StringBuilder().append(ordinal).append('\t').append(column.getName());
          var info = column.getComplexInfo();
          Object value = parent.get(column.getName());
          if(info == null) row.append("\tS\t").append(scalar(value, column.getType()));
          else {
            var key = (ComplexValueForeignKey)value;
            row.append("\tK\t").append(key == null ? "N" : key.get());
            if(key != null) switch(info.getType()) {
              case MULTI_VALUE -> {
                DataType type = ((MultiValueColumnInfoImpl)info).getValueColumn().getType();
                for(SingleValue item : key.getMultiValues()) row.append('\t').append(scalar(item.get(), type));
              }
              case ATTACHMENT -> {
                var flat = ((ComplexColumnInfoImpl<?>)info).getComplexValueForeignKeyColumn().getTable();
                var date = (ColumnImpl)flat.getColumn("FileTimeStamp");
                for(Attachment item : key.getAttachments()) {
                  byte[] bytes = item.getEncodedFileData();
                  row.append("\tI32:").append(item.getId().get()).append('\t').append(scalar(item.getFileUrl(), DataType.TEXT))
                    .append('\t').append(scalar(item.getFileName(), DataType.TEXT)).append('\t').append(scalar(item.getFileType(), DataType.TEXT))
                    .append('\t').append(bytes == null ? "N" : "L:" + bytes.length).append('\t').append(date(item.getFileTimeStampObject(), date))
                    .append('\t').append(scalar(item.getFileFlags(), DataType.LONG));
                }
              }
              case VERSION_HISTORY -> {
                var history = (VersionHistoryColumnInfoImpl)info;
                Column foreign = history.getComplexValueForeignKeyColumn(), identity = history.getPrimaryKeyColumn();
                var date = (ColumnImpl)history.getModifiedDateColumn();
                for(Row item : foreign.getTable()) if(Objects.equals(item.get(foreign.getName()), key.get()))
                  row.append("\tI32:").append(item.get(identity.getName())).append('\t').append(scalar(item.get(history.getValueColumn().getName()), DataType.MEMO))
                    .append('\t').append(date(item.get(date.getName()), date));
              }
              default -> throw new IllegalArgumentException("Unqualified collection");
            }
          }
          System.out.println(row);
        }
        ordinal++;
      }
    }
  }
}
