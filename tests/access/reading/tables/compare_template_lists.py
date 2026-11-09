"""Original stored SOAP list declarations compared with inline XSD and CLI results.

No SOAP service or template application code runs. Independently generated ZIP/XML
fixtures qualify this declared-schema profile, not native Access instantiation.
"""

import argparse
from pathlib import Path
import tempfile
import xml.etree.ElementTree as ET
import zipfile

from compare_template_packages import package
from compare_template_tables import parts, relationships, command, XSD, ACCESS
from compare_error_messages import execute


SOAP = "http://schemas.xmlsoap.org/soap/envelope/"
SP = "http://schemas.microsoft.com/sharepoint/soap/"
LIST_REL = "http://schemas.microsoft.com/office/2007/relationships/ListInstanceDefinition"
PROPERTIES_REL = "http://schemas.microsoft.com/office/2007/relationships/ObjectProperties"
LABEL_ID = "abcdef01-2345-4678-8abc-def012345678"
FIELDS = {
    "ID": "Counter", "Small": "Integer", "DoubleValue": "Number",
    "Money": "Currency", "Flag": "Boolean", "Full_x0020_Name": "Text",
    "Stamp": "DateTime", "Notes": "Note",
}


def node(parent, local, **attributes):
    return ET.SubElement(parent, f"{{{SP}}}{local}", attributes)


def definition():
    root = ET.Element("Root")
    def request(kind):
        envelope = ET.SubElement(root, f"{{{SOAP}}}Envelope")
        body = ET.SubElement(envelope, f"{{{SOAP}}}Body")
        return ET.SubElement(body, f"{{{SP}}}{kind}")
    add = request("AddList")
    node(add, "listName").text = "Sample Table"
    node(add, "templateID").text = "100"
    update = request("UpdateList")
    node(update, "listName").text = "{12345678-1234-4234-8234-123456789abc}"
    node(node(update, "listProperties"), "List", EnableAttachments="FALSE")
    new_fields = node(node(update, "newFields"), "Fields")
    for index, (name, kind) in enumerate(FIELDS.items()):
        if name == "ID":
            continue
        method = node(new_fields, "Method", ID=str(index))
        attributes = {"Name": name, "Type": kind, "Required": "TRUE"}
        if kind == "Text":
            attributes.update(Name="RawLabel", DisplayName="Before Rename", ID=LABEL_ID, MaxLength="32")
        field = node(method, "Field", **attributes)
        node(field, "Default").text = "Never execute a default"
    node(node(new_fields, "Method", ID="temporary"), "Field", Name="Temporary", Type="Text")
    base = node(node(update, "updateFields"), "Fields")
    node(node(base, "Method", ID="base"), "Field", Name="ID", Type="Counter", Required="FALSE", ReadOnly="TRUE")
    update = request("UpdateList")
    node(update, "listProperties")
    node(update, "newFields")
    changed = node(node(update, "updateFields"), "Fields")
    node(node(changed, "Method", ID="label"), "Field", ID="{" + LABEL_ID.upper() + "}", DisplayName="Full Name", MaxLength="20")
    deleted = node(node(update, "deleteFields"), "Fields")
    node(node(deleted, "Method", ID="delete"), "Field", Name="Temporary")
    node(update, "listVersion").text = "1"
    return root


def fixture():
    original = parts()
    data = ET.fromstring(original["data.xml"])
    sequence = data.find(f"{{{XSD}}}schema")[1].find(f"{{{XSD}}}complexType/{{{XSD}}}sequence")
    for field in list(sequence):
        name = field.attrib["name"]
        if name not in FIELDS:
            sequence.remove(field)
        elif name == "Small":
            field.set("type", "x:int")
        elif name == "Full_x0020_Name":
            del field.attrib["type"]
            restriction = ET.SubElement(ET.SubElement(field, f"{{{XSD}}}simpleType"), f"{{{XSD}}}restriction", base="x:string")
            ET.SubElement(restriction, f"{{{XSD}}}maxLength", value="20")
    ET.SubElement(sequence, f"{{{XSD}}}element", name="Notes", type="x:string")
    for row in data.find("dataroot"):
        for value in list(row):
            if value.tag not in FIELDS:
                row.remove(value)
        ET.SubElement(row, "Notes").text = "Data & <inert>"
    original.update({
        "data.xml": ET.tostring(data, encoding="unicode"),
        "definition.xml": ET.tostring(definition(), encoding="unicode"),
        "properties.xml": "<original-opaque-properties/>",
        "template/_rels/main.xml.rels": relationships(("table", LIST_REL, "../definition.xml")),
        "_rels/definition.xml.rels": relationships(("metadata", ACCESS + "object-metadata", "metadata.xml"), ("data", ACCESS + "table-data", "data.xml"), ("properties", PROPERTIES_REL, "properties.xml")),
    })
    original["[Content_Types].xml"] = original["[Content_Types].xml"].replace("</Types>", '<Override PartName="/definition.xml" ContentType="application/xml"/><Override PartName="/properties.xml" ContentType="application/vnd-ms-access.objectproperties+xml"/></Types>')
    return original


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    args = parser.parse_args()
    count = 0
    with tempfile.TemporaryDirectory(prefix="tablecraft-template-lists-") as directory:
        path = Path(directory) / "original.accdt"
        for compression in [zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED]:
            original = fixture()
            path.write_bytes(package(original, compression))
            assert command(args.cli, path, ["tables", str(path), "--count"])[1] == ["Sample Table", "Template", "2"]
            assert len(command(args.cli, path, ["inspect", str(path), "--table", "Sample Table"])) == len(FIELDS) + 1
            assert len(command(args.cli, path, ["rows", str(path), "Sample Table"])) == 3
            assert command(args.cli, path, ["rows", str(path), "--all"])[0] == ["Table", "Sample Table"]
            assert execute(args.cli, "run", "SELECT ID FROM [Sample Table] WHERE Flag", path) == [["1"]]
            assert execute(args.cli, "run", "SELECT Count(*),Sum(ID),Min([Money]) FROM [Sample Table]", path) == [["2", "3", "-0.0001"]]
            assert execute(args.cli, "run", "SELECT A.ID FROM [Sample Table] AS A INNER JOIN [Sample Table] AS B ON A.ID=B.ID ORDER BY A.ID", path) == [["1"], ["2"]]
            assert execute(args.cli, "run", "SELECT Notes FROM [Sample Table] ORDER BY ID", path) == [['"Data & <inert>"'], ['"Data & <inert>"']]
            count += 8
            for attribute, value in [("MaxLength", "19"), ("DisplayName", "Wrong"), ("ID", "invalid")]:
                root = definition()
                changed = root[-1].find(f"{{{SOAP}}}Body/{{{SP}}}UpdateList/{{{SP}}}updateFields/{{{SP}}}Fields/{{{SP}}}Method/{{{SP}}}Field")
                changed.set(attribute, value)
                invalid = {**original, "definition.xml": ET.tostring(root, encoding="unicode")}
                path.write_bytes(package(invalid, compression))
                command(args.cli, path, ["inspect", str(path), "--table", "Sample Table"], success=False)
                execute(args.cli, "run", "SELECT ID FROM [Sample Table]", path, success=False)
                count += 2
            for malformed in ["bad", "2147483648"]:
                invalid = {**original, "data.xml": original["data.xml"].replace("<ID>2</ID>", f"<ID>{malformed}</ID>")}
                path.write_bytes(package(invalid, compression))
                command(args.cli, path, ["inspect", str(path), "--table", "Sample Table"])
                execute(args.cli, "run", "SELECT ID FROM [Sample Table]", path, success=False)
                count += 2
    print(f"Passed {count} original SOAP/XSD template CLI/query controls; native Access unrun.")


if __name__ == "__main__":
    main()
