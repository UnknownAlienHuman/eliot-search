impl RealDataPlane {
    fn replay(&self, mutation: &BridgeMutation) -> Result<Option<MutationReceipt>, BridgeError> {
        let Some(existing) = self.operations.get(&mutation.operation_id) else {
            return Ok(None);
        };
        if existing.canonical_input_digest != mutation.canonical_input_digest {
            return Err(BridgeError::OperationConflict);
        }
        let mut replay = existing.clone();
        replay.replayed = true;
        Ok(Some(replay))
    }

    fn record_mutation(
        &mut self,
        route: CollectionRoute,
        mutation: BridgeMutation,
        affected_ids: Vec<QdrantPointId>,
    ) -> Result<MutationReceipt, BridgeError> {
        if self.operations.len() >= self.limits.max_operation_receipts {
            return Err(BridgeError::MutationTooLarge);
        }
        let receipt = MutationReceipt {
            operation_id: mutation.operation_id.clone(),
            canonical_input_digest: mutation.canonical_input_digest,
            route,
            affected_ids,
            replayed: false,
        };
        self.operations
            .insert(mutation.operation_id, receipt.clone());
        Ok(receipt)
    }
}
