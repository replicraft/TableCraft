use super::support::*;
use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_query::{
    QueryError, Source,
    source::{MultivalueList, MultivalueVisitor, ProjectedRow},
};
use tablecraft_schema::{CollectionMember, Column, ScalarType, SchemaName};

struct Named {
    data: Data<'static>,
    unavailable: Vec<SchemaName>,
    members: Vec<CollectionMember>,
    rows: Vec<Vec<Vec<Value>>>,
    scans: Cell<usize>,
}

impl Named {
    fn new() -> Self {
        Self {
            data: Data::fixture(),
            unavailable: ["B", "A", "Files", "Other"]
                .map(|name| SchemaName::new(name.into()).unwrap())
                .to_vec(),
            members: [("Files", "Code"), ("Other", "Mark"), ("Files", "Weight")]
                .map(|(collection, name)| {
                    CollectionMember::new(
                        collection.into(),
                        Column::new(name.into(), ScalarType::Int32, true).unwrap(),
                    )
                    .unwrap()
                })
                .to_vec(),
            rows: vec![
                vec![
                    vec![Value::Int32(1), Value::Int32(2)],
                    vec![Value::Int32(30)],
                    vec![Value::Int32(100), Value::Int32(200)],
                ],
                vec![vec![], vec![], vec![]],
                vec![vec![Value::Null], vec![], vec![Value::Int32(300)]],
                vec![vec![Value::Int32(4)], vec![], vec![Value::Int32(400)]],
            ],
            scans: Cell::new(0),
        }
    }
}

impl Source for Named {
    fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
        self.data.columns(name)
    }

    fn unavailable_columns(&self, _: &str) -> Result<&[SchemaName], QueryError> {
        assert!(!self.data.active.get());
        Ok(&self.unavailable)
    }

    fn multivalue_columns(&self, name: &str) -> Result<&[Column], QueryError> {
        self.data.multivalue_columns(name)
    }

    fn collection_members(&self, _: &str) -> Result<&[CollectionMember], QueryError> {
        assert!(!self.data.active.get());
        Ok(&self.members)
    }

    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("mixed named members must use the collection capability");
    }

    fn scan_collection_members(
        &self,
        _: &str,
        indices: &[usize],
        values: &[usize],
        members: &[usize],
        visitor: &mut MultivalueVisitor<'_>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        self.data.active.set(true);
        let result = (|| {
            for (parent, members_row) in self.data.tables[0].rows.iter().zip(&self.rows) {
                let lists = values
                    .iter()
                    .map(|index| &parent.lists[*index] as &dyn MultivalueList)
                    .chain(
                        members
                            .iter()
                            .map(|index| &members_row[*index] as &dyn MultivalueList),
                    )
                    .collect::<Vec<_>>();
                visitor(ProjectedRow::select(&parent.scalars, indices)?, &lists)?;
            }
            Ok(())
        })();
        self.data.active.set(false);
        result
    }
}

#[test]
fn displayed_named_members_and_hidden_siblings_use_the_same_child_ordinal() {
    let source = Named::new();
    let sql = "SELECT A,Files.Code FROM L WHERE Files.Weight=200 AND Other.Mark=30";
    let result = execute(sql, &source, false).unwrap();
    assert_rows(
        &result,
        &[vec![list(&source.data.tables[0].rows[0], 0), scalar(2)]],
    );
    assert_eq!(source.scans.get(), 1);

    let mut source = Named::new();
    source.rows[0][2].pop();
    assert_eq!(execute(sql, &source, false).err(), Some(QueryError::Source));
    assert!(!source.data.active.get());
}
