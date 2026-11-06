//! Fixed complete-unit batches, exact point verification and count.

use std::collections::BTreeSet;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use prost::Message;
use qdrant_client::qdrant::{CountPoints, GetPoints};

use super::{Backend, check};
use crate::codec::{decode_id, encode_batch, payload_string, point_id, source_id};
use crate::model::collection_name;
use crate::{BackendError, PointInput, schema};

impl Backend {
    /// Measures the real SDK protobuf batch before a pending page is persisted.
    ///
    /// This pure preflight uses the same private encoder as dispatch, including
    /// the final collection name. It performs no connection or RPC. Durable page
    /// encoding and protobuf request bytes have independent checked ceilings.
    ///
    /// # Errors
    ///
    /// Returns invalid/duplicate input, definite request oversize, allocation,
    /// cancellation or deadline failures before any dispatch.
    pub fn batch_request_bytes(
        name: &str,
        points: &[PointInput<'_>],
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<usize, BackendError> {
        Ok(encode_batch(name, points, deadline, cancelled)?.encoded_len())
    }

    /// Dispatches the exact fixed batch with `InsertOnly` and wait=true.
    ///
    /// A completed acknowledgement is not generation readiness. Publication must
    /// verify actual payload/count/schema and its real text-query canary.
    ///
    /// # Errors
    ///
    /// Returns definite pre-dispatch refusal or `UnknownDispatch` for a possibly
    /// dispatched mutation without a terminal completed acknowledgement.
    pub fn insert_only(
        &self,
        name: &str,
        points: &[PointInput<'_>],
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), BackendError> {
        if points.len() > self.limits.max_batch_points {
            return Err(BackendError::RequestTooLarge);
        }
        let message = encode_batch(name, points, deadline, cancelled)?;
        let request = self.request(message, deadline, cancelled)?;
        let mut client = self.points();
        let reply = self.call(client.upsert(request), deadline, cancelled, true)?;
        schema::completed(reply.result.ok_or(BackendError::UnknownDispatch)?.status)
    }

    /// Reads exact IDs and compares complete expected text/source payload, without vectors.
    ///
    /// Publication supplies the immutable expected page; matching readback does
    /// not clear an earlier unresolved writer or prove mutation terminality.
    ///
    /// # Errors
    ///
    /// Returns input/budget, read/response, exact point mismatch, cancellation
    /// or deadline failures. No source content is returned from this method.
    pub fn verify_points(
        &self,
        name: &str,
        expected: &[PointInput<'_>],
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), BackendError> {
        check(deadline, cancelled)?;
        collection_name(name)?;
        if expected.is_empty() {
            return Ok(());
        }
        if expected.len() > self.limits.max_batch_points {
            return Err(BackendError::RequestTooLarge);
        }
        encode_batch(name, expected, deadline, cancelled)?;
        let ids = expected.iter().map(|point| point_id(point.id)).collect();
        let message = GetPoints {
            collection_name: name.to_owned(),
            ids,
            with_vectors: Some(false.into()),
            with_payload: Some(vec!["source_id", "retrieval_text"].into()),
            ..Default::default()
        };
        let request = self.request(message, deadline, cancelled)?;
        let mut client = self.points();
        let reply = self.call(client.get(request), deadline, cancelled, false)?;
        if reply.result.len() > expected.len() {
            return Err(BackendError::ResponseTooLarge);
        }
        if reply.result.len() != expected.len() {
            return Err(BackendError::PointMismatch);
        }
        let mut seen = BTreeSet::new();
        for point in reply.result {
            check(deadline, cancelled)?;
            let id = decode_id(point.id.as_ref())?;
            if !seen.insert(id) || point.vectors.is_some() || point.payload.len() != 2 {
                return Err(BackendError::PointMismatch);
            }
            let wanted = expected
                .iter()
                .find(|wanted| wanted.id == id)
                .ok_or(BackendError::PointMismatch)?;
            if source_id(&point.payload)? != wanted.source_id
                || payload_string(&point.payload, "retrieval_text")? != wanted.retrieval_text
            {
                return Err(BackendError::PointMismatch);
            }
        }
        Ok(())
    }

    /// Reads the exact full generation population, without a candidate/filter cap.
    ///
    /// # Errors
    ///
    /// Returns input/read/response, cancellation or deadline failures.
    pub fn count_exact(
        &self,
        name: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<u64, BackendError> {
        collection_name(name)?;
        let request = self.request(
            CountPoints {
                collection_name: name.to_owned(),
                exact: Some(true),
                ..Default::default()
            },
            deadline,
            cancelled,
        )?;
        let mut client = self.points();
        let reply = self.call(client.count(request), deadline, cancelled, false)?;
        Ok(reply.result.ok_or(BackendError::MalformedResponse)?.count)
    }
}
