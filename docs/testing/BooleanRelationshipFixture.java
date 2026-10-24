import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.ConstraintViolationException;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.IndexBuilder;
import com.healthmarketscience.jackcess.RelationshipBuilder;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import java.nio.file.Files;
import java.nio.file.Path;

/** Original inert Boolean RI recipe; writes only fresh engineering fixtures. */
public class BooleanRelationshipFixture {
    private static final int PARENTS = 1200;
    private static final int CHILDREN = 1500;

    interface Attempt {
        void run() throws Exception;
    }

    private static void rejected(Attempt action) throws Exception {
        try {
            action.run();
        } catch (ConstraintViolationException expected) {
            return;
        }
        throw new AssertionError("Expected original reference constraint rejection");
    }

    private static Table single(Database database, String name, boolean primary) throws Exception {
        TableBuilder builder = new TableBuilder(name)
            .addColumn(new ColumnBuilder("ID", DataType.LONG))
            .addColumn(new ColumnBuilder("Flag", DataType.BOOLEAN));
        if (primary) {
            builder.addIndex(new IndexBuilder("BoolKey").addColumns("Flag").setPrimaryKey());
        }
        return builder.toTable(database);
    }

    private static Table mixed(Database database, String name, boolean unique) throws Exception {
        TableBuilder builder = new TableBuilder(name)
            .addColumn(new ColumnBuilder("ID", DataType.LONG))
            .addColumn(new ColumnBuilder("Flag", DataType.BOOLEAN))
            .addColumn(new ColumnBuilder("Part", DataType.LONG))
            .addColumn(new ColumnBuilder("Other", DataType.BOOLEAN));
        if (unique) {
            builder.addIndex(new IndexBuilder("MixedKey").addColumns(false, "Flag")
                .addColumns("Part").addColumns(false, "Other").setUnique());
        }
        return builder.toTable(database);
    }

    private static void relationship(
        Database database, Table parent, Table child, String name, boolean composite
    ) throws Exception {
        RelationshipBuilder builder = new RelationshipBuilder(parent, child).addColumns("Flag", "Flag");
        if (composite) {
            builder.addColumns("Part", "Part").addColumns("Other", "Other");
        }
        builder.setReferentialIntegrity().setName(name).toRelationship(database);
    }

    public static void main(String[] args) throws Exception {
        if (args.length < 2 || args.length > 3) {
            throw new IllegalArgumentException("Expected fresh path, V2016/V2019 and optional --null-orphan");
        }
        Path path = Path.of(args[0]);
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Fresh output required");
        }
        Database.FileFormat format = switch (args[1]) {
            case "V2016" -> Database.FileFormat.V2016;
            case "V2019" -> Database.FileFormat.V2019;
            default -> throw new IllegalArgumentException("Unsupported profile");
        };
        boolean orphan = args.length == 3 && args[2].equals("--null-orphan");
        if (args.length == 3 && !orphan) {
            throw new IllegalArgumentException("Unknown fixture mode");
        }

        try (Database database = new DatabaseBuilder(path).setFileFormat(format).create()) {
            database.setEvaluateExpressions(false);
            Table empty = single(database, "EmptyParents", true);
            Table emptyChild = single(database, "EmptyChildren", false);
            relationship(database, empty, emptyChild, "EmptyKeys", false);
            rejected(() -> emptyChild.addRow(1, false));
            rejected(() -> emptyChild.addRow(2, true));
            if (orphan) {
                // Observed reference behavior: input NULL bypasses RI but becomes
                // stored false. This is deliberately INVALID read-audit evidence.
                emptyChild.addRow(3, null);
                if (!Boolean.FALSE.equals(emptyChild.iterator().next().get("Flag"))) {
                    throw new AssertionError("Expected stored false, not a nullable Boolean");
                }
                return;
            }

            Table parent = single(database, "BoolParents", true);
            Table child = single(database, "BoolChildren", false);
            relationship(database, parent, child, "BoolKeys", false);
            parent.addRow(1, true);
            parent.addRow(2, false);
            for (int index = 0; index < CHILDREN; index++) {
                child.addRow(index, index % 7 == 0 ? null : index % 3 == 0);
            }
            rejected(() -> parent.addRow(3, null));
            Table unique = single(database, "BoolUniqueChildren", true);
            relationship(database, parent, unique, "UniqueBoolKeys", false);
            unique.addRow(1, true);
            unique.addRow(2, null);
            rejected(() -> unique.addRow(3, false));

            Table only = single(database, "TrueParents", true);
            Table onlyChild = single(database, "TrueChildren", false);
            relationship(database, only, onlyChild, "TrueKeys", false);
            only.addRow(1, true);
            for (int index = 0; index < CHILDREN; index++) {
                onlyChild.addRow(index, true);
            }
            rejected(() -> onlyChild.addRow(CHILDREN, false));

            Table mixedParent = mixed(database, "MixedParents", true);
            Table mixedChild = mixed(database, "MixedChildren", false);
            relationship(database, mixedParent, mixedChild, "MixedKeys", true);
            for (int index = 0; index < PARENTS; index++) {
                mixedParent.addRow(index, index % 2 == 0, index, index % 3 == 0);
            }
            int nullableIdentity = PARENTS;
            for (boolean flag : new boolean[] {false, true}) {
                for (boolean other : new boolean[] {false, true}) {
                    mixedParent.addRow(nullableIdentity++, flag, null, other);
                }
            }
            for (int index = 0; index < CHILDREN; index++) {
                int key = index % PARENTS;
                mixedChild.addRow(index, key % 2 == 0, key, key % 3 == 0);
            }
            mixedChild.addRow(CHILDREN, null, null, null);
            mixedChild.addRow(CHILDREN + 1, true, null, true);
            rejected(() -> mixedChild.addRow(CHILDREN + 2, false, -1, false));

            Table mixedUnique = mixed(database, "MixedUniqueChildren", true);
            relationship(database, mixedParent, mixedUnique, "UniqueMixedKeys", true);
            mixedUnique.addRow(1, false, null, false);
            mixedUnique.addRow(2, true, null, true);
            mixedUnique.addRow(3, true, 0, true);
            rejected(() -> mixedUnique.addRow(4, null, null, null));
        }
        System.out.println("Boolean RI, NULL-to-false, exact partial tuples and uniqueness assertions passed");
    }
}
