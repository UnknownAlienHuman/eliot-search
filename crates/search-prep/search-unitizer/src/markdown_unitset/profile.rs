//! Bounded pure profile identity binds literal production and fixture definitions.
use super::codec::{array, bytes, text};
use super::{
    MAX_MARKDOWN_UNITIZER_LIMITS, MarkdownUnitizationError as Error, MarkdownUnitizerLimits,
};
use search_contracts::{
    Blake3Digest32, CanonicalDigestDomain, CanonicalValue, DigestInputLimit, ProfileId,
};

pub(super) const FORMAT: &str = "markdown-structural-unit-manifest/v4";
pub(super) const PROFILE_FORMAT: &str = "md-structural-unitizer/v4";
pub(super) const SEMANTICS: &str = "raw-v3-partition/whole-node-observation/mapped-values/v1";

/// Immutable internally identified structural profile; selectors are checked at ingress.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarkdownUnitizerProfile {
    pub(super) raw_id: ProfileId,
    pub(super) markdown_id: Blake3Digest32,
    pub(super) limits: MarkdownUnitizerLimits,
    pub(super) algorithm: Blake3Digest32,
    pub(super) fixture: Blake3Digest32,
    id: ProfileId,
}
impl MarkdownUnitizerProfile {
    /// Construct a bounded pure profile, without a clock or caller-supplied fixture proof.
    pub fn new(
        raw_id: ProfileId,
        markdown_id: Blake3Digest32,
        limits: MarkdownUnitizerLimits,
    ) -> Result<Self, Error> {
        limits.validate(MAX_MARKDOWN_UNITIZER_LIMITS)?;
        let definitions = [
            (
                "eliot/raw/md-unitizer-model/v4",
                include_bytes!("model.rs").as_slice(),
            ),
            (
                "eliot/raw/md-unitizer-profile/v4",
                include_bytes!("profile.rs").as_slice(),
            ),
            (
                "eliot/raw/md-unitizer-budget/v4",
                include_bytes!("budget.rs").as_slice(),
            ),
            (
                "eliot/raw/md-unitizer-build/v4",
                include_bytes!("build.rs").as_slice(),
            ),
            (
                "eliot/raw/md-unitizer-codec/v4",
                include_bytes!("codec.rs").as_slice(),
            ),
            (
                "eliot/raw/md-unitizer-decode/v4",
                include_bytes!("decode.rs").as_slice(),
            ),
        ];
        let rows = definitions
            .into_iter()
            .map(|(domain, source)| {
                definition(domain, source, 256 * 1024).and_then(|digest| bytes(digest.as_bytes()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let algorithm = pure_hash("eliot/cbor/md-unitizer-algorithm/v4", array(rows)?)?;
        let fixture = definition(
            "eliot/raw/md-unitizer-fixture/v4",
            include_bytes!("fixtures.rs"),
            64 * 1024,
        )?;
        let value = descriptor(&raw_id, markdown_id, limits, algorithm, fixture)?;
        let digest = pure_hash("eliot/cbor/md-unitizer-profile/v4", value)?;
        let id = ProfileId::new(format!("md-unitizer-v4-{digest}")).map_err(|_| Error::Codec)?;
        Ok(Self {
            raw_id,
            markdown_id,
            limits,
            algorithm,
            fixture,
            id,
        })
    }
    /// Full shared-owner-computed profile identity.
    pub const fn id(&self) -> &ProfileId {
        &self.id
    }
    /// Exact selected positive profile ceilings.
    pub const fn limits(&self) -> MarkdownUnitizerLimits {
        self.limits
    }
    pub(super) fn value(&self) -> Result<CanonicalValue, Error> {
        descriptor(
            &self.raw_id,
            self.markdown_id,
            self.limits,
            self.algorithm,
            self.fixture,
        )
    }
}
fn descriptor(
    raw: &ProfileId,
    markdown: Blake3Digest32,
    limits: MarkdownUnitizerLimits,
    algorithm: Blake3Digest32,
    fixture: Blake3Digest32,
) -> Result<CanonicalValue, Error> {
    array(vec![
        text(PROFILE_FORMAT)?,
        text(raw.as_str())?,
        bytes(markdown.as_bytes())?,
        array(
            limits
                .dimensions()
                .into_iter()
                .map(|n| super::budget::number(n).map(CanonicalValue::U64))
                .collect::<Result<_, _>>()?,
        )?,
        bytes(algorithm.as_bytes())?,
        bytes(fixture.as_bytes())?,
        text(SEMANTICS)?,
    ])
}
fn definition(domain: &str, source: &[u8], cap: usize) -> Result<Blake3Digest32, Error> {
    if super::budget::add(super::budget::add(domain.len(), 1)?, source.len())? > cap {
        return Err(Error::CanonicalLimit);
    }
    search_contracts::blake3_raw(
        &CanonicalDigestDomain::parse(domain).map_err(|_| Error::Codec)?,
        source,
        DigestInputLimit::new(cap).map_err(|_| Error::CanonicalLimit)?,
    )
    .map_err(|_| Error::CanonicalLimit)
}
fn pure_hash(domain: &str, value: CanonicalValue) -> Result<Blake3Digest32, Error> {
    let CanonicalValue::Array(rows) = value else {
        return Err(Error::Codec);
    };
    search_contracts::blake3_canonical_array_with_usage(
        &CanonicalDigestDomain::parse(domain).map_err(|_| Error::Codec)?,
        rows.len(),
        rows.into_vec().into_iter().map(Ok),
        DigestInputLimit::new(16 * 1024).map_err(|_| Error::CanonicalLimit)?,
    )
    .map(search_contracts::CanonicalBlake3Output::digest)
    .map_err(|_| Error::CanonicalLimit)
}
