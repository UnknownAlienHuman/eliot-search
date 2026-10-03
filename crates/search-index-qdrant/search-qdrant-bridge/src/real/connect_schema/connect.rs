impl RealDataPlane {
    /// Connects to the exact supervisor-bound loopback server admitted by an
    /// executed qualification gate, using its purpose-bound API-key lease.
    /// The provider revalidates current process, owner, endpoint and lease
    /// identity before each RPC. Admission requires a protected
    /// `list_collections` request; a successful health check alone is not
    /// sufficient.
    ///
    /// A dead endpoint fails with [`BridgeError::TransportFailed`] (definite:
    /// nothing was sent); a version/build drift fails with
    /// [`BridgeError::CapabilityReceiptMismatch`].
    pub async fn connect(
        endpoint: &LiveEndpoint,
        supervisor: QdrantConnectionBinding,
        auth_lease: QdrantApiKeyLease,
        gate: QualifiedGate,
        limits: BridgeLimits,
    ) -> Result<Self, BridgeError> {
        let limits = limits.validate()?;
        auth_lease.validate_binding(supervisor)?;
        if !supervisor.matches_live_endpoint(endpoint) {
            return Err(BridgeError::SupervisorReceiptMismatch);
        }
        let mut built_client = None;
        auth_lease.with_secret(|api_key| {
            built_client = Some(
                Qdrant::from_url(&endpoint.grpc_url())
                    .api_key(api_key)
                    .timeout(Duration::from_secs(10))
                    .connect_timeout(Duration::from_secs(5))
                    .skip_compatibility_check()
                    .build()
                    .map_err(|_| BridgeError::TransportFailed)?,
            );
            Ok(())
        })?;
        let client = built_client.ok_or(BridgeError::AuthenticationInvalid)?;
        auth_lease.validate()?;
        let reply = tokio::time::timeout(Duration::from_secs(15), client.health_check())
            .await
            .map_err(|_| BridgeError::TransportFailed)?
            .map_err(map_read_error)?;
        if reply.version != QUALIFIED_SERVER_VERSION
            || reply.version != gate.server_version()
            || reply
                .commit
                .as_deref()
                .is_none_or(|commit| !commit.starts_with(QUALIFIED_SERVER_BUILD))
        {
            return Err(BridgeError::CapabilityReceiptMismatch);
        }
        auth_lease.validate()?;
        tokio::time::timeout(Duration::from_secs(15), client.list_collections())
            .await
            .map_err(|_| BridgeError::TransportFailed)?
            .map_err(map_read_error)?;
        Ok(Self {
            client,
            auth_lease,
            gate,
            limits,
            schemas: BTreeMap::new(),
            operations: BTreeMap::new(),
        })
    }

    /// Admitted gate bound to this data plane (evidence, not transport).
    #[must_use]
    pub const fn gate(&self) -> &QualifiedGate {
        &self.gate
    }

    /// Revalidates the current supervisor binding and secret lease before
    /// each vendor dispatch.
    pub(crate) fn authorize_dispatch(&self) -> Result<(), BridgeError> {
        self.auth_lease.validate()
    }
}
