//! Guarded fresh catalog publication, with no SQL execution or identity guessing.
use crate::{
    Error, QueryCreatedReopen, Response,
    controls::{SCOPE, action_review_budget, check},
    document::Document,
};
use tablecraft_ace::{NativeImage, read_saved_query_definition};
use tablecraft_core::limits::Resource;

impl Document {
    pub(crate) fn open_query_created(
        witness: &QueryCreatedReopen,
        revision: u64,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(Self, Response), Error> {
        let (document, response) = Self::open_with_expected(
            witness.source_path(),
            &mut String::new(),
            revision,
            cancelled,
            Some(witness.expected_bytes()),
        )
        .map_err(|error| match error {
            Error::SourceChanged | Error::CreatedSourceChanged => Error::QueryCreatedSourceChanged,
            other => other,
        })?;
        check(cancelled)?;
        let mut work = action_review_budget(witness.expected_bytes().len())?;
        // NO_CLONE: Fresh catalog verification borrows the Document's immutable
        // source independently; sharing adds no image payload copy.
        let image = NativeImage::capture_shared_retained(
            document.edits.image_source().clone(),
            SCOPE,
            &mut work,
            cancelled,
        )?;
        let created = read_saved_query_definition(
            &mut image.reader(),
            SCOPE,
            witness.query_name(),
            &mut work,
            cancelled,
        )?;
        work.charge(Resource::Steps, witness.query_name().len() as u64)?;
        if created.origin().native_id() != witness.native_query_id()
            || created.origin().type_code() != 5
            || created.origin().flags() != 0
            || created.object().name().as_str() != witness.query_name()
        {
            return Err(Error::QueryCreatedSourceChanged);
        }
        witness.verify_storage(&image, &mut work, cancelled)?;
        check(cancelled)?;
        Ok((document, response))
    }
}
