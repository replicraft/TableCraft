"""Model retained-list member composition from immutable original getters.

Inline derived SQL is TableCraft's engineering profile. Stored getters establish
values, not native Access execution of these queries or locale equivalence.
"""

COLUMNS = ("Bytes", "Shorts", "Ints", "Singles", "Doubles", "Guids", "Decimals", "Texts")
IEEE_COLUMNS = frozenset(("Singles", "Doubles"))


def cases(model):
    identifiers = sorted(model["Ints"])

    def children(column, identifier):
        return model[column][identifier] or ["N"]

    def expanded(column, population):
        return [
            [f"I32:{identifier}", model[column][identifier], member]
            for identifier in population
            for member in children(column, identifier)
        ]

    headings = ["ID", "A", "V"]
    for column in COLUMNS:
        # The first IEEE parent contains nonfinite values. Their whole lists
        # remain representable, but exposing those members is a scalar refusal.
        population = identifiers[1:] if column in IEEE_COLUMNS else identifiers
        predicate = " WHERE ID>=2" if column in IEEE_COLUMNS else ""
        yield (
            f"derived-{column.lower()}-members",
            f"SELECT Q.ID,Q.Whole AS A,Q.Whole.Value AS V FROM "
            f"(SELECT ID,{column} AS Whole FROM Multivalues{predicate}) Q ORDER BY Q.ID",
            headings,
            expanded(column, population),
            None,
        )

    for column in ("Singles", "Doubles"):
        yield (
            f"derived-{column.lower()}-whole-with-independent-member",
            f"SELECT Q.ID,Q.Whole AS A,Q.Finite.Value AS V FROM "
            f"(SELECT ID,{column} AS Whole,Bytes AS Finite FROM Multivalues) Q ORDER BY Q.ID",
            headings,
            [
                [f"I32:{identifier}", model[column][identifier], member]
                for identifier in identifiers
                for member in children("Bytes", identifier)
            ],
            None,
        )
        yield (
            f"derived-{column.lower()}-visible-nonfinite",
            f"SELECT Q.Whole,Q.Whole.Value FROM "
            f"(SELECT {column} AS Whole FROM Multivalues) Q",
            [],
            [],
            "non-finite",
        )

    first = identifiers[0]
    first_members = children("Ints", first)
    yield (
        "repeated-aliases-share-one-expansion",
        "SELECT Q.A,Q.A.Value AS L,Q.B.Value AS R FROM "
        "(SELECT Ints AS A,Ints AS B FROM Multivalues WHERE ID=1) Q",
        ["A", "L", "R"],
        [[model["Ints"][first], member, member] for member in first_members],
        None,
    )
    yield (
        "null-and-empty-repeated-aliases",
        "SELECT Q.ID,Q.A,Q.A.Value AS L,Q.B.Value AS R FROM "
        "(SELECT ID,Ints AS A,Ints AS B FROM Multivalues WHERE ID IN (2,3)) Q ORDER BY Q.ID",
        ["ID", "A", "L", "R"],
        [
            [f"I32:{identifier}", model["Ints"][identifier], member, member]
            for identifier in identifiers if identifier in (2, 3)
            for member in children("Ints", identifier)
        ],
        None,
    )
    yield (
        "independent-derived-occurrences-form-product",
        "SELECT a.Whole AS A,a.Whole.Value AS L,b.Whole.Value AS R FROM "
        "(SELECT Ints AS Whole FROM Multivalues WHERE ID=1) a,"
        "(SELECT Ints AS Whole FROM Multivalues WHERE ID=1) b",
        ["A", "L", "R"],
        [
            [model["Ints"][first], left, right]
            for left in first_members for right in first_members
        ],
        None,
    )
    yield (
        "completed-inner-read-reexpands-retained-list",
        "SELECT Q.Whole AS A,Q.Child AS L,Q.Whole.Value AS R FROM "
        "(SELECT Ints AS Whole,Ints.Value AS Child FROM Multivalues WHERE ID=1) Q",
        ["A", "L", "R"],
        [
            [model["Ints"][first], inner, outer]
            for inner in first_members for outer in first_members
        ],
        None,
    )
    yield (
        "displayed-member-filter-keeps-complete-list",
        "SELECT Q.ID,Q.Whole AS A,Q.Whole.Value AS V FROM "
        "(SELECT ID,Ints AS Whole FROM Multivalues) Q "
        "WHERE Q.Whole.Value=0 ORDER BY Q.ID",
        headings,
        [row for row in expanded("Ints", identifiers) if row[2] == "I32:0"],
        None,
    )
    selected = {
        identifier for identifier in identifiers if "I32:0" in model["Ints"][identifier]
    }
    yield (
        "hidden-independent-criteria-retain-whole-and-visible-members",
        "SELECT Q.ID,Q.Whole AS A,Q.Visible.Value AS V FROM "
        "(SELECT ID,Ints AS Whole,Bytes AS Visible FROM Multivalues) Q "
        "WHERE Q.Whole.Value=0 ORDER BY Q.ID",
        headings,
        [
            [f"I32:{identifier}", model["Ints"][identifier], member]
            for identifier in identifiers if identifier in selected
            for member in children("Bytes", identifier)
        ],
        None,
    )
    outer = []
    for identifier in identifiers:
        target = identifier - 1
        if target in identifiers:
            outer.extend(
                [f"I32:{identifier}", model["Ints"][target], member]
                for member in children("Ints", target)
            )
        else:
            outer.append([f"I32:{identifier}", "N", "N"])
    yield (
        "derived-absent-null-and-empty-parent",
        "SELECT Q.ID,Q.Whole AS A,Q.Whole.Value AS V FROM "
        "(SELECT a.ID,b.Ints AS Whole FROM Multivalues a "
        "LEFT JOIN Multivalues b ON a.ID=b.ID+1) Q ORDER BY Q.ID",
        headings,
        outer,
        None,
    )
    yield (
        "derived-text-ordinal-filter-preserves-duplicates",
        "SELECT Q.ID,Q.Whole AS A,Q.Whole.Value AS V FROM "
        "(SELECT ID,Texts AS Whole FROM Multivalues) Q "
        "WHERE Q.Whole.Value='Red' ORDER BY Q.ID",
        headings,
        [row for row in expanded("Texts", identifiers) if row[2] == "T526564"],
        None,
    )

    for name, sql, reason in [
        (
            "eager-unused-inner-error",
            "SELECT Q.Whole,Q.Whole.Value FROM "
            "(SELECT Ints AS Whole,1/0 AS Boom FROM Multivalues) Q",
            "division by zero",
        ),
        (
            "distinct-refusal",
            "SELECT DISTINCT Q.Whole,Q.Whole.Value FROM "
            "(SELECT Ints AS Whole FROM Multivalues) Q",
            "not implemented",
        ),
        (
            "union-refusal",
            "SELECT Q.Whole,Q.Whole.Value FROM (SELECT Ints AS Whole FROM Multivalues) Q "
            "UNION ALL SELECT Q.Whole,Q.Whole.Value FROM "
            "(SELECT Ints AS Whole FROM Multivalues) Q",
            "not implemented",
        ),
        (
            "list-sort-refusal",
            "SELECT Q.Whole,Q.Whole.Value FROM "
            "(SELECT Ints AS Whole FROM Multivalues) Q ORDER BY Q.Whole",
            "not implemented",
        ),
        (
            "whole-list-scalar-operator-refusal",
            "SELECT Q.Whole+1 FROM (SELECT Ints AS Whole FROM Multivalues) Q",
            "not implemented",
        ),
    ]:
        yield name, sql, [], [], reason


if __name__ == "__main__":
    from collection_query_replay import replay

    replay(
        cases,
        "Original-getter retained-list member composition; inline derived SQL is an "
        "engineering profile, not native Access SQL qualification",
        __file__,
        ["crates/query/tablecraft-query/src/execution/collections/joined/derived.rs"],
        [
            "--collation", "ordinal",
            "--native-steps", "20000000",
            "--execution-bytes", "16777216",
            "--execution-rows", "100000",
            "--execution-steps", "10000000",
        ],
    )
