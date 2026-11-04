use super::{Error, Response, Table};
use std::{
    io::{Cursor, Read, Seek, SeekFrom},
    path::Path,
};
use tablecraft_access::{AccessInput, NativeBrowser, NativeSource};
use tablecraft_ace::{EncryptedSource, Header, InspectError, NativeReadSource, inspect_header};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{ExpressionValue, Value},
};
use tablecraft_io::SharedFile;
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, QueryLimits, execute_with_schema_discovery};
use tablecraft_sql::{ParseLimits, parse_query};
use zeroize::Zeroizing;

const SNAPSHOT_BYTES: u64 = 64 * 1024 * 1024;
const NATIVE_BYTES: u64 = 512 * 1024 * 1024;
const NATIVE_ROWS: u64 = 100_000;
const NATIVE_STEPS: u64 = 5_000_000;
const COPY_CHUNK: usize = 64 * 1024;
const PAGE_ROWS: u32 = 100;
const OUTPUT_BYTES: u64 = 8 * 1024 * 1024;
const SQL_BYTES: usize = 64 * 1024;
const SCOPE: CatalogScope = CatalogScope::new(1);

type Input = AccessInput<Cursor<Vec<u8>>>;

enum Source {
    Plain(Input),
    Encrypted(EncryptedSource<Input>),
}

impl Read for Source {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(source) => source.read(buffer),
            Self::Encrypted(source) => source.read(buffer),
        }
    }
}

impl Seek for Source {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        match self {
            Self::Plain(source) => source.seek(position),
            Self::Encrypted(source) => source.seek(position),
        }
    }
}

impl NativeReadSource for Source {
    fn qualify_header(&self, header: Header) -> Result<Header, InspectError> {
        match self {
            Self::Plain(source) => source.qualify_header(header),
            Self::Encrypted(source) => source.qualify_header(header),
        }
    }

    fn decode_native_page(
        &mut self,
        number: u32,
        page: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), InspectError> {
        match self {
            Self::Plain(source) => source.decode_native_page(number, page, budget, cancelled),
            Self::Encrypted(source) => source.decode_native_page(number, page, budget, cancelled),
        }
    }
}

pub(super) struct Document {
    source: Source,
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(NATIVE_BYTES, NATIVE_ROWS, NATIVE_STEPS)
}

fn check(cancelled: &dyn Fn() -> bool) -> Result<(), Error> {
    if cancelled() {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}

impl Document {
    pub(super) fn open(
        path: &Path,
        password: &mut String,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(Self, Response), Error> {
        // Take credentials immediately, including on path or snapshot failures.
        let password = Zeroizing::new(std::mem::take(password));
        let mut work = budget();
        check(cancelled)?;
        let mut file = SharedFile::open(path)?;
        let length = file.length()?;
        if length > SNAPSHOT_BYTES {
            return Err(Error::SnapshotLimit);
        }
        work.charge(Resource::Bytes, length)?;
        let size = usize::try_from(length).map_err(|_| Error::SnapshotLimit)?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(size)?;
        bytes.resize(size, 0);
        for chunk in bytes.chunks_mut(COPY_CHUNK) {
            tablecraft_io::read_exact(&mut file, chunk, &mut work, cancelled)?;
        }
        if file.length()? != length {
            return Err(Error::SourceChanged);
        }
        file.close()?;
        check(cancelled)?;
        let (mut document, format) =
            Self::from_bytes(bytes, password.as_str(), &mut work, cancelled)?;
        let mut browser = NativeBrowser::new(&mut document.source, SCOPE, work, cancelled);
        let inventory = browser.tables()?;
        let tables = inventory
            .entries()
            .map(|entry| Table {
                name: entry.object().name().as_str().to_owned(),
                system: entry.is_system(),
                storage: format!("{:?}", entry.storage()),
            })
            .collect();
        check(cancelled)?;
        let title = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned();
        Ok((
            document,
            Response::Opened {
                title,
                format,
                tables,
            },
        ))
    }

    pub(super) fn from_bytes(
        bytes: Vec<u8>,
        password: &str,
        work: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(Self, String), Error> {
        let mut input = AccessInput::open(Cursor::new(bytes), work, cancelled)?;
        let deployment = input.is_deployment();
        let header = inspect_header(&mut input, work, cancelled)?;
        let format = format!(
            "{:?}{}",
            header.format(),
            if deployment {
                " · deployment package"
            } else {
                ""
            }
        );
        let source = if header.encoded_pages() {
            let password = (!password.is_empty()).then_some(password);
            Source::Encrypted(EncryptedSource::open(input, password, work, cancelled)?)
        } else {
            Source::Plain(input)
        };
        Ok((Self { source }, format))
    }

    pub(super) fn page(
        &mut self,
        table: &str,
        offset: u32,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        let mut browser = NativeBrowser::new(&mut self.source, SCOPE, budget(), cancelled);
        let request = tablecraft_access::TablePageRequest::new(offset, PAGE_ROWS)?;
        let page = browser.page(table, request)?;
        let mut output = Presentation::new(cancelled);
        let columns = page
            .columns()
            .iter()
            .map(|column| output.text(column.name()))
            .collect::<Result<_, _>>()?;
        let rows = page
            .rows()
            .iter()
            .map(|row| {
                row.iter()
                    .map(|value| output.value(value))
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<_, _>>()?;
        Ok(Response::Data {
            table: table.to_owned(),
            offset,
            columns,
            rows,
            has_more: !page.reached_end(),
        })
    }

    pub(super) fn query(
        &mut self,
        sql: &str,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        check(cancelled)?;
        if sql.len() > SQL_BYTES {
            return Err(Error::SqlLimit);
        }
        let parsed = parse_query(sql, ParseLimits::default())?;
        let source = NativeSource::new(&mut self.source, SCOPE, budget(), cancelled)?;
        let result = execute_with_schema_discovery(
            parsed.query(),
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(cancelled),
                limits: QueryLimits {
                    bytes: 16 * 1024 * 1024,
                    rows: 10_000,
                    steps: 1_000_000,
                },
                ..Default::default()
            },
        )?;
        let mut output = Presentation::new(cancelled);
        let columns = result
            .columns()
            .iter()
            .map(|name| output.text(name))
            .collect::<Result<_, _>>()?;
        let rows = result
            .rows()
            .iter()
            .map(|row| {
                row.iter()
                    .map(|value| match value {
                        ExpressionValue::Value(value) => output.value(value),
                        ExpressionValue::Empty => output.text("Empty"),
                        ExpressionValue::Missing => output.text("Missing"),
                        ExpressionValue::Error(code) => output.text(&format!("#ERROR({code})")),
                        ExpressionValue::StringBytes(_) => Err(Error::UnpublishedString),
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<_, _>>()?;
        Ok(Response::Data {
            table: "Query results".to_owned(),
            offset: 0,
            columns,
            rows,
            has_more: false,
        })
    }
}

struct Presentation<'a> {
    work: ResourceBudget,
    cancelled: &'a dyn Fn() -> bool,
}

impl<'a> Presentation<'a> {
    fn new(cancelled: &'a dyn Fn() -> bool) -> Self {
        Self {
            work: ResourceBudget::new(OUTPUT_BYTES, 0, 100_000),
            cancelled,
        }
    }

    fn text(&mut self, text: &str) -> Result<String, Error> {
        check(self.cancelled)?;
        self.work.charge(Resource::Steps, 1)?;
        self.work.charge(Resource::Bytes, text.len() as u64)?;
        Ok(text.to_owned())
    }

    fn value(&mut self, value: &Value) -> Result<String, Error> {
        let text = match value {
            Value::Text(text) => return self.text(text),
            Value::Null => "NULL".to_owned(),
            Value::Boolean(value) => value.to_string(),
            Value::Byte(value) => value.to_string(),
            Value::Int16(value) => value.to_string(),
            Value::Int32(value) => value.to_string(),
            Value::Int64(value) => value.to_string(),
            Value::Single(value) => value.to_float().to_string(),
            Value::Double(value) => value.to_float().to_string(),
            Value::Currency(value) => value.to_string(),
            Value::Decimal(value) => value.to_string(),
            Value::Binary(bytes) => {
                check(self.cancelled)?;
                let length = bytes
                    .len()
                    .checked_mul(2)
                    .and_then(|value| value.checked_add(2))
                    .ok_or(Error::BinaryLimit)?;
                self.work.charge(Resource::Bytes, length as u64)?;
                let mut text = String::with_capacity(length);
                text.push_str("0x");
                for byte in bytes {
                    check(self.cancelled)?;
                    self.work.charge(Resource::Steps, 1)?;
                    use std::fmt::Write;
                    write!(text, "{byte:02x}").map_err(|_| Error::Formatting)?;
                }
                return Ok(text);
            }
            Value::Guid(value) => {
                let b = value.to_bytes();
                format!(
                    "{{{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}}}",
                    b[0],
                    b[1],
                    b[2],
                    b[3],
                    b[4],
                    b[5],
                    b[6],
                    b[7],
                    b[8],
                    b[9],
                    b[10],
                    b[11],
                    b[12],
                    b[13],
                    b[14],
                    b[15]
                )
            }
            Value::DateTime(value) => {
                let date = value.calendar_rounded_to_nanos()?;
                format!(
                    "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:09}",
                    date.year(),
                    date.month(),
                    date.day(),
                    date.hour(),
                    date.minute(),
                    date.second(),
                    date.subsec_nanosecond()
                )
            }
            Value::DateTimeExtended(value) => {
                let (year, month, day) = value.date();
                let (hour, minute, second, fraction) = value.time();
                format!(
                    "{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}.{fraction:07}"
                )
            }
        };
        self.text(&text)
    }
}
