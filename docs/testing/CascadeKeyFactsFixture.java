import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.IndexBuilder;
import com.healthmarketscience.jackcess.RelationshipBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import com.healthmarketscience.jackcess.impl.IndexData;
import com.healthmarketscience.jackcess.impl.RowIdImpl;
import com.healthmarketscience.jackcess.impl.TableImpl;
import java.lang.reflect.Method;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.HexFormat;
import java.util.List;
import java.util.TimeZone;

/** Original same-table unindexed payload recipe; never executes stored content. */
public final class CascadeKeyFactsFixture {
    private static final int MEMO_UNITS = 64 * 1024;
    private static final int OLE_BYTES = 128 * 1024;
    private static final int SLOT_OFFSET_MASK = 0x1fff;
    private static final List<String> TABLES = List.of("Parents", "Children");

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setEnforceForeignKeys(true);
        database.setTimeZone(TimeZone.getTimeZone("UTC"));
        database.setLinkResolver((source, token) -> {
            throw new AssertionError("No linked database is authorized");
        });
        if (!database.isEnforceForeignKeys()) {
            throw new AssertionError("Foreign keys must remain enforced");
        }
    }

    private static byte[] binary(int identity) {
        byte[] bytes = new byte[OLE_BYTES];
        for (int index = 0; index < bytes.length; index++) {
            bytes[index] = (byte) (index * 31 + index / 256 + identity);
        }
        return bytes;
    }

    private static String memo(int identity) {
        return Character.toString((char) ('a' + identity % 20)).repeat(MEMO_UNITS);
    }

    private static void generate(Path output, String profile) throws Exception {
        if (Files.exists(output) || !List.of("V2016", "V2019").contains(profile)) {
            throw new IllegalArgumentException("Expected fresh ACE16 output/profile");
        }
        try (Database database = new DatabaseBuilder(output)
                .setFileFormat(Database.FileFormat.valueOf(profile)).create()) {
            inert(database);
            Table parents = new TableBuilder("Parents")
                .addColumn(new ColumnBuilder("ColdMemo", DataType.MEMO))
                .addColumn(new ColumnBuilder("Id", DataType.LONG))
                .addColumn(new ColumnBuilder("ColdBytes", DataType.OLE))
                .addColumn(new ColumnBuilder("Tenant", DataType.LONG))
                .addIndex(new IndexBuilder("PrimaryPair").addColumns("Tenant", "Id").setPrimaryKey())
                .addIndex(new IndexBuilder("PairAlias").addColumns("Tenant", "Id").setUnique())
                .toTable(database);
            Table children = new TableBuilder("Children")
                .addColumn(new ColumnBuilder("Id", DataType.LONG))
                .addColumn(new ColumnBuilder("ColdMemo", DataType.MEMO))
                .addColumn(new ColumnBuilder("ParentId", DataType.LONG))
                .addColumn(new ColumnBuilder("ColdBytes", DataType.OLE))
                .addColumn(new ColumnBuilder("TenantFk", DataType.LONG))
                .addIndex(new IndexBuilder("PrimaryId").addColumns("Id").setPrimaryKey())
                .addIndex(new IndexBuilder("ParentPair").addColumns("TenantFk", "ParentId"))
                .addIndex(new IndexBuilder("ParentPairAlias").addColumns("TenantFk", "ParentId"))
                .toTable(database);
            parents.addRow(memo(1), 10, binary(1), 7);
            parents.addRow(memo(2), 20, binary(2), 7);
            parents.addRow("", 10, new byte[0], 8);
            children.addRow(100, memo(3), 10, binary(3), 7);
            children.addRow(101, memo(4), 10, binary(4), 7);
            children.addRow(102, memo(5), 20, binary(5), 7);
            children.addRow(103, memo(6), 10, binary(6), 8);
            children.addRow(104, null, null, null, null);
            children.addRow(105, "", 10, new byte[0], 8);
            new RelationshipBuilder(parents, children)
                .setName("ParentPair_Children")
                .addColumns("Tenant", "TenantFk")
                .addColumns("Id", "ParentId")
                .setReferentialIntegrity()
                .setCascadeDeletes()
                .toRelationship(database);
        }
    }

    private static String hash(byte[] value) throws Exception {
        return value == null ? "NULL" : HexFormat.of().formatHex(
            MessageDigest.getInstance("SHA-256").digest(value));
    }

    private static int number(Object object, String name) throws Exception {
        Method method = IndexData.class.getDeclaredMethod(name);
        method.setAccessible(true);
        return ((Number) method.invoke(object)).intValue();
    }

    private static void inspect(Path input) throws Exception {
        byte[] raw = Files.readAllBytes(input);
        System.out.printf("FILE\t%d\t%s%n", raw.length, hash(raw));
        try (Database database = new DatabaseBuilder(input).setReadOnly(true).open()) {
            inert(database);
            for (var relationship : database.getRelationships()) {
                System.out.printf("RELATION\t%s\t%s\t%s\t%s\t%s\t%s\t%s%n",
                    relationship.getName(), relationship.getFromTable().getName(),
                    relationship.getToTable().getName(), relationship.hasReferentialIntegrity(),
                    relationship.cascadeUpdates(), relationship.cascadeDeletes(),
                    relationship.cascadeNullOnDelete());
                for (int position = 0; position < relationship.getFromColumns().size(); position++) {
                    System.out.printf("PAIR\t%d\t%s\t%s%n", position,
                        relationship.getFromColumns().get(position).getName(),
                        relationship.getToColumns().get(position).getName());
                }
            }
            for (String name : TABLES) {
                TableImpl table = (TableImpl) database.getTable(name);
                var format = table.getFormat();
                System.out.printf("TABLE\t%s\t%d\t%d\t%d%n", name,
                    table.getTableDefPageNumber(), table.getRowCount(), format.PAGE_SIZE);
                if (table.getProperties().iterator().hasNext()) {
                    throw new AssertionError("Unexpected table properties");
                }
                for (var column : table.getColumns()) {
                    if (column.getProperties().iterator().hasNext()) {
                        throw new AssertionError("Unexpected column properties");
                    }
                    System.out.printf("COLUMN\t%s\t%s\t%s\t%d%n", name,
                        column.getName(), column.getType(), column.getColumnIndex());
                }
                for (var index : table.getIndexes()) {
                    IndexData data = index.getIndexData();
                    System.out.printf("INDEX\t%s\t%s\t%d\t%d\t%d\t%d%n", name,
                        index.getName(), data.getIndexDataNumber(),
                        number(data, "getRootPageNumber"), data.getOwnedPageCount(), data.getEntryCount());
                }
                int ordinal = 0;
                for (Row row : table) {
                    RowIdImpl location = (RowIdImpl) row.getId();
                    int base = location.getPageNumber() * format.PAGE_SIZE;
                    ByteBuffer page = ByteBuffer.wrap(raw, base, format.PAGE_SIZE).slice()
                        .order(ByteOrder.LITTLE_ENDIAN);
                    int slot = location.getRowNumber();
                    int offset = format.OFFSET_ROW_START + slot * Short.BYTES;
                    int word = Short.toUnsignedInt(page.getShort(offset));
                    if ((word & ~SLOT_OFFSET_MASK) != 0) {
                        throw new AssertionError("Expected ordinary original root");
                    }
                    int start = word & SLOT_OFFSET_MASK;
                    int end = slot == 0 ? format.PAGE_SIZE
                        : Short.toUnsignedInt(page.getShort(offset - Short.BYTES)) & SLOT_OFFSET_MASK;
                    String text = row.getString("ColdMemo");
                    byte[] bytes = row.getBytes("ColdBytes");
                    System.out.printf("ROW\t%s\t%d\t%s\t%s\t%s\t%d\t%s\t%d\t%s\t%d\t%d\t%s%n",
                        name, ordinal++, row.getInt("Id"),
                        row.get(name.equals("Parents") ? "Tenant" : "TenantFk"),
                        name.equals("Parents") ? row.getInt("Id") : row.get("ParentId"),
                        text == null ? -1 : text.length(),
                        hash(text == null ? null : text.getBytes(StandardCharsets.UTF_16LE)),
                        bytes == null ? -1 : bytes.length, hash(bytes),
                        location.getPageNumber(), slot,
                        HexFormat.of().formatHex(raw, base + start, base + end));
                }
            }
        }
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 2) {
            throw new IllegalArgumentException("Expected fresh path/profile or --inspect/path");
        }
        if (args[0].equals("--inspect")) {
            inspect(Path.of(args[1]));
        } else {
            generate(Path.of(args[0]), args[1]);
        }
    }
}
