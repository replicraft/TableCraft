"""Original typed ACCDT snapshots, Python XML/ZIP models and CLI read/query controls.

Only TableCraft executes SQL. These are independently laid-out original fixtures,
not Microsoft Access-created templates or native application qualification.
"""

import argparse
import csv
import io
from pathlib import Path
import subprocess
import tempfile
import xml.etree.ElementTree as ET
import zipfile

from compare_template_packages import package, TYPES, ROOT
from compare_error_messages import execute


XSD = "http://www.w3.org/2001/XMLSchema"
OBJECT = "http://schemas.microsoft.com/office/access/2005/04/template/object-metadata"
REL = "http://schemas.openxmlformats.org/package/2006/relationships"
ACCESS = "http://schemas.microsoft.com/office/access/2005/04/template/"
ET.register_namespace("x", XSD)
FIELDS = [
    ("ID", "int", "1", "2", 3),
    ("Tiny", "byte", "-128", "127", 2),
    ("Octet", "unsignedByte", "0", "255", 17),
    ("Small", "short", "-32768", "32767", 2),
    ("UnsignedSmall", "unsignedShort", "0", "65535", 3),
    ("UnsignedLong", "unsignedInt", "0", "4294967295", 20),
    ("Large", "long", "-9223372036854775808", "9223372036854775807", 20),
    ("FloatValue", "float", "1.5", "-2.5", 4),
    ("DoubleValue", "double", "1.25", "-2.25", 5),
    ("Money", "decimal", "12.3400", "-0.0001", 14),
    ("Flag", "boolean", "true", "0", 11),
    ("Full_x0020_Name", "string", " A&🦀 ", "", 8),
    ("Stamp", "dateTime", "2025-01-01T12:00:00", "1899-12-29T06:00:00", 7),
    ("Bytes", "base64Binary", "AQI=", "", 8209),
    ("Hex", "hexBinary", "0102", "", 8209),
]


def relationships(*entries):
    root = ET.Element("Relationships", xmlns=REL)
    for identity, kind, target in entries:
        ET.SubElement(root, "Relationship", Id=identity, Type=kind, Target=target)
    return ET.tostring(root, encoding="unicode")


def parts():
    root = ET.Element("root")
    schema = ET.SubElement(root, f"{{{XSD}}}schema")
    data_root = ET.SubElement(schema, f"{{{XSD}}}element", name="dataroot")
    complex_type = ET.SubElement(data_root, f"{{{XSD}}}complexType")
    sequence = ET.SubElement(complex_type, f"{{{XSD}}}sequence")
    ET.SubElement(sequence, f"{{{XSD}}}element", ref="Sample_x0020_Table", minOccurs="0", maxOccurs="unbounded")
    row = ET.SubElement(schema, f"{{{XSD}}}element", name="Sample_x0020_Table")
    complex_type = ET.SubElement(row, f"{{{XSD}}}complexType")
    sequence = ET.SubElement(complex_type, f"{{{XSD}}}sequence")
    for name, kind, *_ in FIELDS:
        ET.SubElement(sequence, f"{{{XSD}}}element", name=name, type=f"x:{kind}")
    instance = ET.SubElement(root, "dataroot")
    for index in [2, 3]:
        row = ET.SubElement(instance, "Sample_x0020_Table")
        for field in FIELDS:
            ET.SubElement(row, field[0]).text = field[index]
    metadata = ET.Element(f"{{{OBJECT}}}AccessObject")
    ET.SubElement(metadata, f"{{{OBJECT}}}Type").text = "Table"
    ET.SubElement(metadata, f"{{{OBJECT}}}Name").text = "Sample Table"
    types = TYPES.replace("</Types>", '<Override PartName="/metadata.xml" ContentType="application/xml"/><Override PartName="/data.xml" ContentType="application/xml"/></Types>')
    return {
        "[Content_Types].xml": types,
        "_rels/.rels": ROOT,
        "template/main.xml": '<Template xmlns="' + ACCESS + 'start"><FlipRightToLeft>0</FlipRightToLeft><PerformLocalizationFixup>0</PerformLocalizationFixup><RequiredAccessVersion>14</RequiredAccessVersion><AccessServicesVersion>14.0</AccessServicesVersion><Type>Desktop</Type></Template>',
        "template/_rels/main.xml.rels": relationships(("table", ACCESS + "object", "../definition.xml")),
        "definition.xml": "Opaque original object body; must not run or be parsed",
        "_rels/definition.xml.rels": relationships(("metadata", ACCESS + "object-metadata", "metadata.xml"), ("data", ACCESS + "table-data", "data.xml")),
        "metadata.xml": ET.tostring(metadata, encoding="unicode"),
        "data.xml": ET.tostring(root, encoding="unicode"),
    }


def command(cli, path, args, success=True):
    before = path.read_bytes()
    output = subprocess.run([str(cli), *args, "--format", "tsv"], capture_output=True, check=False)
    assert path.read_bytes() == before
    if not success:
        assert output.returncode == 2, output.stderr
        assert not output.stdout
        return None
    assert output.returncode == 0, output.stderr
    return list(csv.reader(io.StringIO(output.stdout.decode()), delimiter="\t"))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    args = parser.parse_args()
    count = 0
    with tempfile.TemporaryDirectory(prefix="tablecraft-template-tables-") as directory:
        path = Path(directory) / "original.accdt"
        for compression in [zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED]:
            original = parts()
            path.write_bytes(package(original, compression))
            assert command(args.cli, path, ["tables", str(path), "--count"]) == [["Name", "Kind", "Row Count"], ["Sample Table", "Template", "2"]]
            columns = command(args.cli, path, ["inspect", str(path), "--table", "Sample Table"])
            assert len(columns) == len(FIELDS) + 1
            assert ["Full Name", "Text", "FALSE"] in columns
            rows = command(args.cli, path, ["rows", str(path), "Sample Table", "--limit", "1", "--offset", "1"])
            assert len(rows) == 2 and rows[1][0] == "2"
            assert command(args.cli, path, ["rows", str(path), "--all"])[0] == ["Table", "Sample Table"]
            assert len(command(args.cli, path, ["inspect", str(path), "--catalog"])) == 2
            count += 5
            names = [field[0].replace("_x0020_", " ") for field in FIELDS]
            sql = "SELECT " + ",".join(f"VarType([{name}])" for name in names) + " FROM [Sample Table] ORDER BY ID"
            expected = [str(field[4]) for field in FIELDS]
            assert execute(args.cli, "run", sql, path) == [expected, expected]
            assert execute(args.cli, "run", "SELECT Count(*),Sum(ID),Min([Money]),Max([Money]) FROM [Sample Table]", path) == [["2", "3", "-0.0001", "12.3400"]]
            assert execute(args.cli, "run", "SELECT A.ID FROM [Sample Table] AS A INNER JOIN [Sample Table] AS B ON A.ID=B.ID WHERE B.Flag", path) == [["1"]]
            assert execute(args.cli, "run", "SELECT ID FROM [Sample Table] WHERE [Full Name]=' A&🦀 '", path, controls=("--collation", "ordinal")) == [["1"]]
            scalar_names = [field[0] for field in FIELDS[:11]]
            sql = "SELECT " + ",".join(f"[{name}]" for name in scalar_names) + " FROM [Sample Table] ORDER BY ID"
            expected_values = [[field[index] for field in FIELDS[:11]] for index in [2, 3]]
            expected_values[0][-1] = "TRUE"
            expected_values[1][-1] = "FALSE"
            assert execute(args.cli, "run", sql, path) == expected_values
            assert execute(args.cli, "run", "SELECT [Bytes],[Hex] FROM [Sample Table] ORDER BY ID", path) == [["0x0102", "0x0102"], ["0x", "0x"]]
            count += 6
            for value in ["bad", "2147483648", "1.5"]:
                invalid = {**original, "data.xml": original["data.xml"].replace("<ID>2</ID>", f"<ID>{value}</ID>")}
                path.write_bytes(package(invalid, compression))
                # Metadata lookup succeeds without parsing the bad record.
                command(args.cli, path, ["inspect", str(path), "--table", "Sample Table"])
                for operation in [["tables", str(path), "--count"], ["rows", str(path), "Sample Table"]]:
                    command(args.cli, path, operation, success=False)
                    count += 1
                execute(args.cli, "run", "SELECT ID FROM [Sample Table]", path, success=False)
                count += 2
            path.write_bytes(package(original, compression))
            execute(args.cli, "run", "SELECT ID FROM [Sample Table]", path, controls=("--collation", "database"), success=False)
            count += 1
    print(f"Passed {count} original typed-template CLI/query controls; native Access unrun.")


if __name__ == "__main__":
    main()
