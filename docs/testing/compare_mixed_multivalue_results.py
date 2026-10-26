"""Compare whole-list sidecars and displayed children with original getters."""


def cases(model):
    identifiers = sorted(model["Ints"])

    def children(column, identifier):
        return model[column][identifier] or ["N"]

    def expanded(whole, member):
        return [
            [f"I32:{identifier}", model[whole][identifier], value]
            for identifier in identifiers for value in children(member, identifier)
        ]

    headings = ["ID", "A", "V"]
    yield (
        "same-field-complete-lists",
        "SELECT ID,Ints AS A,Ints.Value AS V FROM Multivalues ORDER BY ID",
        headings, expanded("Ints", "Ints"), None,
    )
    yield (
        "same-field-aligned-filter",
        "SELECT ID,Ints AS A,Ints.Value AS V FROM Multivalues WHERE Ints.Value=0 ORDER BY ID",
        headings, [row for row in expanded("Ints", "Ints") if row[2] == "I32:0"], None,
    )
    yield (
        "text-duplicates-null-empty",
        "SELECT ID,Texts AS A,Texts.Value AS V FROM Multivalues ORDER BY ID",
        headings, expanded("Texts", "Texts"), None,
    )
    yield (
        "independent-displayed-collection",
        "SELECT ID,Ints AS A,Bytes.Value AS V FROM Multivalues ORDER BY ID",
        headings, expanded("Ints", "Bytes"), None,
    )
    selected = {identifier for identifier in identifiers if "I32:0" in model["Ints"][identifier]}
    yield (
        "independent-hidden-membership",
        "SELECT ID,Ints AS A,Bytes.Value AS V FROM Multivalues WHERE Ints.Value=0 ORDER BY ID",
        headings, [row for row in expanded("Ints", "Bytes") if int(row[0][4:]) in selected], None,
    )
    yield (
        "derived-full-list-filtered-child",
        "SELECT q.ID,q.Whole AS A,q.One AS V FROM "
        "(SELECT ID,Ints AS Whole,Ints.Value AS One FROM Multivalues) q "
        "WHERE q.One=0 ORDER BY q.ID",
        headings, [row for row in expanded("Ints", "Ints") if row[2] == "I32:0"], None,
    )
    left = []
    for identifier in identifiers:
        target = identifier - 1
        if target in identifiers:
            for value in children("Ints", target):
                left.append([f"I32:{identifier}", f"I32:{target}", model["Ints"][target], value])
        else:
            left.append([f"I32:{identifier}", "N", "N", "N"])
    outer_headings = ["L", "R", "A", "V"]
    yield (
        "left-stored-null-empty-absent",
        "SELECT a.ID AS L,b.ID AS R,b.Ints AS A,b.Ints.Value AS V FROM Multivalues a "
        "LEFT JOIN Multivalues b ON a.ID=b.ID+1 ORDER BY a.ID",
        outer_headings, left, None,
    )
    right = [
        [f"I32:{identifier}", f"I32:{identifier + 1}", model["Ints"][identifier], value]
        for identifier in identifiers if identifier + 1 in identifiers
        for value in children("Ints", identifier)
    ]
    right.append(["N", f"I32:{identifiers[0]}", "N", "N"])
    yield (
        "right-unmatched-tail",
        "SELECT a.ID AS L,b.ID AS R,a.Ints AS A,a.Ints.Value AS V FROM Multivalues a "
        "RIGHT JOIN Multivalues b ON a.ID+1=b.ID",
        outer_headings, right, None,
    )
    yield (
        "nested-expanded-join",
        "SELECT a.ID AS L,b.ID AS R,b.Ints AS A,b.Ints.Value AS V FROM Multivalues a "
        "LEFT JOIN (Multivalues b INNER JOIN Multivalues c ON b.ID=c.ID) "
        "ON a.ID=b.ID+1 ORDER BY a.ID",
        outer_headings, left, None,
    )
    def member_order(value):
        return value != "N", int(value[4:]) if value != "N" else 0

    descending = [
        [f"I32:{identifier}", model["Ints"][identifier], value]
        for identifier in reversed(identifiers)
        for value in sorted(children("Ints", identifier), key=member_order, reverse=True)
    ]
    yield (
        "scalar-sort-top",
        "SELECT TOP 3 ID,Ints AS A,Ints.Value AS V FROM Multivalues ORDER BY ID DESC,V DESC",
        headings, descending[:3], None,
    )
    yield (
        "text-aligned-filter",
        "SELECT ID,Texts AS A,Texts.Value AS V FROM Multivalues WHERE Texts.Value='Red' ORDER BY ID",
        headings, [row for row in expanded("Texts", "Texts") if row[2] == "T526564"], None,
    )
    yield (
        "independent-displayed-product",
        "SELECT ID,Ints AS A,Bytes.Value AS B,Shorts.Value AS C FROM Multivalues ORDER BY ID",
        ["ID", "A", "B", "C"], [
            [f"I32:{identifier}", model["Ints"][identifier], first, second]
            for identifier in identifiers
            for first in children("Bytes", identifier)
            for second in children("Shorts", identifier)
        ], None,
    )
    yield (
        "raw-ieee-list-with-independent-finite-member",
        "SELECT ID,Singles AS A,Bytes.Value AS V FROM Multivalues ORDER BY ID",
        headings, expanded("Singles", "Bytes"), None,
    )
    for name, sql, message in [
        ("visible-nonfinite", "SELECT Singles,Singles.Value FROM Multivalues", "non-finite"),
        ("eager-filter-error", "SELECT Ints,Ints.Value FROM Multivalues WHERE 1/0=0", "division by zero"),
        ("distinct-refusal", "SELECT DISTINCT Ints,Ints.Value FROM Multivalues", "not implemented"),
        ("union-refusal", "SELECT Ints,Ints.Value FROM Multivalues UNION ALL SELECT Ints,Ints.Value FROM Multivalues", "not implemented"),
        ("list-sort-refusal", "SELECT Ints,Ints.Value FROM Multivalues ORDER BY Ints", "not implemented"),
    ]:
        yield name, sql, [], [], message


if __name__ == "__main__":
    from collection_query_replay import replay

    replay(
        cases,
        "Mixed whole-list/displayed-child output using exact original getter values; no native Access SQL execution",
        __file__,
        ["crates/query/tablecraft-query/src/execution/collections/joined/table.rs"],
        ["--collation", "ordinal"],
    )
