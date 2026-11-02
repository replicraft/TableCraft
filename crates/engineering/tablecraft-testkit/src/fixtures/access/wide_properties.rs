//! Independent literal Wide-MR2 properties for working fixture consumers.
// Original tiny framing, authored independently of the decoder and its constants.
// These bytes carry no application macro body or third-party XML.
pub fn text(value: &str) -> Vec<u8> {
    value.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

pub fn name(value: &str) -> Vec<u8> {
    let bytes = text(value);
    let mut output = (bytes.len() as u16).to_le_bytes().to_vec();
    output.extend(bytes);
    output
}

pub fn block(kind: u16, body: &[u8]) -> Vec<u8> {
    let mut output = ((body.len() + 6) as u32).to_le_bytes().to_vec();
    output.extend(kind.to_le_bytes());
    output.extend(body);
    output
}

pub fn names(values: &[&str]) -> Vec<u8> {
    let mut body = Vec::new();
    for value in values {
        body.extend(name(value));
    }
    block(128, &body)
}

pub fn property(index: u16, flags: u8, native_type: u8, value: &[u8]) -> Vec<u8> {
    let mut output = ((value.len() + 12) as u32).to_le_bytes().to_vec();
    output.extend([flags, native_type]);
    output.extend(index.to_le_bytes());
    output.extend((value.len() as u32).to_le_bytes());
    output.extend(value);
    output
}

pub fn group(kind: u16, properties: &[Vec<u8>]) -> Vec<u8> {
    let mut body = vec![10, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    for property in properties {
        body.extend(property);
    }
    block(kind, &body)
}

pub fn bag(blocks: &[Vec<u8>]) -> Vec<u8> {
    let mut output = b"MR2\0".to_vec();
    for block in blocks {
        output.extend(block);
    }
    output
}

pub fn golden() -> Vec<u8> {
    bag(&[
        names(&["ValidateUpdate", "AfterDelete"]),
        group(3, &[property(0, 1, 12, &text("BeforeChange"))]),
        group(4, &[property(1, 1, 12, &text("inert"))]),
    ])
}

/// Explicit event properties in original caller order, without evaluating XML.
pub fn event_properties(properties: &[(&str, &str)]) -> Vec<u8> {
    let property_names = properties.iter().map(|(name, _)| *name).collect::<Vec<_>>();
    let values = properties
        .iter()
        .enumerate()
        .map(|(index, (_, xml))| property(index as u16, 1, 12, &text(xml)))
        .collect::<Vec<_>>();
    bag(&[names(&property_names), group(3, &values)])
}
