use sha2::{Digest, Sha256};

use super::FixtureRepository;

impl FixtureRepository {
    pub fn add_accepted_contracts_handoff(&self, valid_signature: bool) -> (String, String) {
        let accepted_commit = self.tagged_head();
        let prefix = format!(
            concat!(
                "schema_version = 1\n",
                "record_kind = \"package_handoff_v1\"\n",
                "status = \"ACCEPTED\"\n\n",
                "[identity]\n",
                "handoff_id = \"contracts-001\"\n",
                "operation_id = \"{}\"\n",
                "package = \"search-contracts\"\n",
                "stage = \"W0\"\n",
                "accepted_at = \"2026-08-31T00:00:00Z\"\n\n",
                "[accepted_code]\n",
                "base_commit = \"{}\"\n",
                "final_commit = \"{}\"\n",
                "changed_files_digest = \"{}\"\n\n",
                "[public_surface]\n",
                "api_manifest_ref = \"artifact:contracts-api\"\n",
                "api_schema_digest = \"{}\"\n",
                "configuration_digest = \"ABSENT\"\n",
                "fixture_digest_set = []\n",
                "error_reason_digest = \"{}\"\n\n",
            ),
            "3".repeat(64),
            accepted_commit,
            accepted_commit,
            "4".repeat(64),
            "1".repeat(64),
            "2".repeat(64),
        );
        let digest = if valid_signature {
            format!("{:x}", Sha256::digest(prefix.as_bytes()))
        } else {
            "0".repeat(64)
        };
        let text = format!(
            concat!(
                "{}",
                "[signature]\n",
                "record_sha256 = \"{}\"\n",
                "integration_signature_ref = \"signature:contracts\"\n",
            ),
            prefix, digest,
        );
        let path = "swarm/handoffs/search-contracts/contracts-001.toml";
        self.write_text(path, &text);
        (path.to_owned(), self.commit("accepted contracts handoff"))
    }
}
