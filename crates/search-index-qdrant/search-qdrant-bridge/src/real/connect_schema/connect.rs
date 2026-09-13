impl RealDataPlane {
    /// Connects to a disposable loopback server admitted by an executed
    /// qualification gate and rechecks the live server identity.
    ///
    /// A dead endpoint fails with [`BridgeError::TransportFailed`] (definite:
    /// nothing was sent); a version/build drift fails with
    /// [`BridgeError::CapabilityReceiptMismatch`].
    pub async fn connect(
        endpoint: &LiveEndpoint,
        gate: QualifiedGate,
        limits: BridgeLimits,
    ) -> Result<Self, BridgeError> {
        let limits = limits.validate()?;
        let client = Qdrant::from_url(&endpoint.grpc_url())
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5))
            .skip_compatibility_check()
            .build()
            .map_err(|_| BridgeError::TransportFailed)?;
        let reply = tokio::time::timeout(
            Duration::from_secs(15),
            client.health_check(),
        )
        .await
        .map_err(|_| BridgeError::TransportFailed)?
        .map_err(|_| BridgeError::TransportFailed)?;
        if reply.version != QUALIFIED_SERVER_VERSION
            || reply.version != gate.server_version()
            || reply
                .commit
                .as_deref()
                .is_none_or(|commit| !commit.starts_with(QUALIFIED_SERVER_BUILD))
        {
            return Err(BridgeError::CapabilityReceiptMismatch);
        }
        Ok(Self {
            client,
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
}
