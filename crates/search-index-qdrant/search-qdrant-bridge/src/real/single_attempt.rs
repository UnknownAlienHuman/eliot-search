// qdrant-client 1.19.0 with_points_client/with_collections_client retry some
// ambiguous statuses. Canonical mutations use the SDK-generated unary services
// directly: one invocation, then the durable owner resolves any unknown outcome.
use qdrant_client::qdrant::{
    CollectionOperationResponse as SingleCollectionReply,
    PointsOperationResponse as SinglePointsReply, UpdateCollection,
    collections_client::CollectionsClient as SingleCollectionsClient,
    points_client::PointsClient as SinglePointsClient,
};
use tonic::transport::{Channel as SingleMutationChannel, Endpoint as SingleMutationEndpoint};

struct SingleAttemptTransport {
    points: SinglePointsClient<SingleMutationChannel>,
    collections: SingleCollectionsClient<SingleMutationChannel>,
}

impl SingleAttemptTransport {
    fn new(endpoint: &LiveEndpoint) -> Result<Self, BridgeError> {
        let channel = SingleMutationEndpoint::from_shared(endpoint.grpc_url())
            .map_err(|_| BridgeError::TransportFailed)?
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .connect_lazy();
        Ok(Self {
            points: SinglePointsClient::new(channel.clone()),
            collections: SingleCollectionsClient::new(channel),
        })
    }
}

impl RealDataPlane {
    fn canonical_rpc_request<T>(
        &self,
        message: T,
        context: &canonical::CanonicalRequestContext,
    ) -> Result<tonic::Request<T>, BridgeError> {
        context.remaining()?;
        self.authorize_dispatch()?;
        let mut request = tonic::Request::new(message);
        self.auth_lease.with_secret(|key| {
            let value = key
                .parse()
                .map_err(|_| BridgeError::AuthenticationInvalid)?;
            request.metadata_mut().insert("api-key", value);
            Ok(())
        })?;
        request.set_timeout(context.remaining()?.min(Duration::from_secs(10)));
        Ok(request)
    }

    async fn canonical_upsert_once(
        &self,
        message: UpsertPoints,
        context: &canonical::CanonicalRequestContext,
    ) -> Result<SinglePointsReply, BridgeError> {
        let request = self.canonical_rpc_request(message, context)?;
        self.single_attempt
            .points
            .clone()
            .upsert(request)
            .await
            .map(tonic::Response::into_inner)
            .map_err(|_| BridgeError::MutationOutcomeUnknown)
    }

    async fn canonical_close_once(
        &self,
        message: SetPayloadPoints,
        context: &canonical::CanonicalRequestContext,
    ) -> Result<SinglePointsReply, BridgeError> {
        let request = self.canonical_rpc_request(message, context)?;
        self.single_attempt
            .points
            .clone()
            .set_payload(request)
            .await
            .map(tonic::Response::into_inner)
            .map_err(|_| BridgeError::MutationOutcomeUnknown)
    }

    async fn canonical_delete_once(
        &self,
        message: DeletePoints,
        context: &canonical::CanonicalRequestContext,
    ) -> Result<SinglePointsReply, BridgeError> {
        let request = self.canonical_rpc_request(message, context)?;
        self.single_attempt
            .points
            .clone()
            .delete(request)
            .await
            .map(tonic::Response::into_inner)
            .map_err(|_| BridgeError::MutationOutcomeUnknown)
    }

    async fn canonical_create_index_once(
        &self,
        message: CreateFieldIndexCollection,
        context: &canonical::CanonicalRequestContext,
    ) -> Result<SinglePointsReply, BridgeError> {
        let request = self.canonical_rpc_request(message, context)?;
        self.single_attempt
            .points
            .clone()
            .create_field_index(request)
            .await
            .map(tonic::Response::into_inner)
            .map_err(|_| BridgeError::MutationOutcomeUnknown)
    }

    async fn canonical_create_once(
        &self,
        message: CreateCollection,
        context: &canonical::CanonicalRequestContext,
    ) -> Result<SingleCollectionReply, BridgeError> {
        let request = self.canonical_rpc_request(message, context)?;
        self.single_attempt
            .collections
            .clone()
            .create(request)
            .await
            .map(tonic::Response::into_inner)
            .map_err(|_| BridgeError::MutationOutcomeUnknown)
    }

    async fn canonical_update_once(
        &self,
        message: UpdateCollection,
        context: &canonical::CanonicalRequestContext,
    ) -> Result<SingleCollectionReply, BridgeError> {
        let request = self.canonical_rpc_request(message, context)?;
        self.single_attempt
            .collections
            .clone()
            .update(request)
            .await
            .map(tonic::Response::into_inner)
            .map_err(|_| BridgeError::MutationOutcomeUnknown)
    }
}

#[cfg(test)]
mod single_attempt_tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use std::{
        convert::Infallible,
        future::{Ready, ready},
        task::{Context, Poll},
    };
    use tonic::codegen::{Service, http};

    #[derive(Clone)]
    struct AppliedThenUnavailable(Arc<AtomicUsize>);

    impl Service<http::Request<tonic::body::Body>> for AppliedThenUnavailable {
        type Response = http::Response<tonic::body::Body>;
        type Error = Infallible;
        type Future = Ready<Result<Self::Response, Self::Error>>;

        fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, _: http::Request<tonic::body::Body>) -> Self::Future {
            self.0.fetch_add(1, Ordering::SeqCst);
            let response = http::Response::builder()
                .status(200)
                .header("content-type", "application/grpc")
                .header("grpc-status", "14")
                .body(tonic::body::Body::empty())
                .expect("fixed synthetic response");
            ready(Ok(response))
        }
    }

    #[tokio::test]
    async fn generated_mutation_service_does_not_retry_an_ambiguous_status() {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut client = SinglePointsClient::new(AppliedThenUnavailable(Arc::clone(&calls)));
        let error = client
            .upsert(UpsertPoints {
                collection_name: "synthetic".to_owned(),
                wait: Some(true),
                ordering: Some(strong_ordering()),
                ..Default::default()
            })
            .await
            .expect_err("first ambiguous response must escape");
        assert_eq!(error.code(), tonic::Code::Unavailable);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
