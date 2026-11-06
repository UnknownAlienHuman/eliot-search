//! Fixed schema creation/readback and terminal whole-collection deletion.

use std::sync::atomic::AtomicBool;
use std::time::Instant;

use qdrant_client::qdrant::{CollectionExistsRequest, DeleteCollection, GetCollectionInfoRequest};

use super::Backend;
use crate::model::collection_name;
use crate::{BackendError, schema};

impl Backend {
    /// Creates a fresh one-shard vectorless collection and its fixed scoring index.
    ///
    /// # Errors
    ///
    /// Returns ordinary failures only before collection creation succeeds. After
    /// that success, every index/request/readback failure is `UnknownDispatch`,
    /// preserving the pending target obligation. An existing name is not adopted.
    pub fn create_generation(
        &self,
        name: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), BackendError> {
        collection_name(name)?;
        let mut collections = self.collections();
        let request = self.request(schema::create_collection(name), deadline, cancelled)?;
        let created = self.call(collections.create(request), deadline, cancelled, true)?;
        if !created.result {
            return Err(BackendError::BackendRefused);
        }
        self.request(schema::create_text_index(name), deadline, cancelled)
            .and_then(|request| {
                let mut points = self.points();
                let indexed = self.call(
                    points.create_field_index(request),
                    deadline,
                    cancelled,
                    true,
                )?;
                schema::completed(indexed.result.ok_or(BackendError::UnknownDispatch)?.status)
            })
            .and_then(|()| self.verify_schema(name, deadline, cancelled))
            .map_err(|_| BackendError::UnknownDispatch)
    }

    /// Verifies actual vectorless collection and complete fixed text-index parameters.
    ///
    /// # Errors
    ///
    /// Returns actual connection/read, schema, cancellation or deadline failures.
    pub fn verify_schema(
        &self,
        name: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), BackendError> {
        collection_name(name)?;
        let mut collections = self.collections();
        let request = self.request(
            GetCollectionInfoRequest {
                collection_name: name.to_owned(),
            },
            deadline,
            cancelled,
        )?;
        let reply = self.call(collections.get(request), deadline, cancelled, false)?;
        schema::verify(&reply.result.ok_or(BackendError::MalformedResponse)?)
    }

    /// Observes exact logical collection existence without creating or adopting it.
    ///
    /// # Errors
    ///
    /// Returns validation, transport/response, cancellation or deadline failures.
    pub fn exists(
        &self,
        name: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<bool, BackendError> {
        collection_name(name)?;
        let mut collections = self.collections();
        let request = self.request(
            CollectionExistsRequest {
                collection_name: name.to_owned(),
            },
            deadline,
            cancelled,
        )?;
        let reply = self.call(
            collections.collection_exists(request),
            deadline,
            cancelled,
            false,
        )?;
        Ok(reply.result.ok_or(BackendError::MalformedResponse)?.exists)
    }

    /// Deletes one publication-authorized whole generation and verifies logical absence.
    ///
    /// Publication must first prove no current/pending/live/unresolved user can
    /// target it. Absence does not establish physical erasure or writer quiescence.
    ///
    /// # Errors
    ///
    /// Returns validation, unresolved mutation dispatch, refusal/readback,
    /// cancellation or deadline failures. No point-delete operation exists here.
    pub fn delete_generation(
        &self,
        name: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), BackendError> {
        collection_name(name)?;
        if !self.exists(name, deadline, cancelled)? {
            return Ok(());
        }
        let mut collections = self.collections();
        let request = self.request(
            DeleteCollection {
                collection_name: name.to_owned(),
                ..Default::default()
            },
            deadline,
            cancelled,
        )?;
        let deleted = self.call(collections.delete(request), deadline, cancelled, true)?;
        if !deleted.result || self.exists(name, deadline, cancelled)? {
            return Err(BackendError::BackendRefused);
        }
        Ok(())
    }
}
