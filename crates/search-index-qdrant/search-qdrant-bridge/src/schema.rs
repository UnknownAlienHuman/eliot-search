//! The one fixed physical text-index profile; no profile registry.

use qdrant_client::qdrant::{
    CollectionInfo, CollectionStatus, CreateCollection, CreateFieldIndexCollection, FieldType,
    Memory, PayloadIndexParams, PayloadSchemaType, ShardingMethod, TextIndexParams,
    TextIndexParamsBuilder, TextScoringType, TokenizerType, UpdateStatus, payload_index_params,
    stemming_algorithm, vectors_config,
};

use crate::BackendError;

pub const TEXT_FIELD: &str = "retrieval_text";

pub fn create_collection(name: &str) -> CreateCollection {
    CreateCollection {
        collection_name: name.to_owned(),
        shard_number: Some(1),
        replication_factor: Some(1),
        write_consistency_factor: Some(1),
        sharding_method: Some(ShardingMethod::Auto.into()),
        ..Default::default()
    }
}

pub fn text_params() -> TextIndexParams {
    TextIndexParamsBuilder::new(TokenizerType::Code)
        .lowercase(true)
        .ascii_folding(false)
        .phrase_matching(false)
        .disabled_stemmer()
        .enable_hnsw(false)
        .memory(Memory::Pinned)
        .bm25_scoring()
        .build()
}

pub fn create_text_index(name: &str) -> CreateFieldIndexCollection {
    CreateFieldIndexCollection {
        collection_name: name.to_owned(),
        field_name: TEXT_FIELD.to_owned(),
        field_type: Some(FieldType::Text.into()),
        field_index_params: Some(PayloadIndexParams {
            index_params: Some(payload_index_params::IndexParams::TextIndexParams(
                text_params(),
            )),
        }),
        wait: Some(true),
        ..Default::default()
    }
}

pub fn verify(info: &CollectionInfo) -> Result<(), BackendError> {
    if info.status != i32::from(CollectionStatus::Green) {
        return Err(BackendError::BackendRefused);
    }
    let params = info
        .config
        .as_ref()
        .and_then(|config| config.params.as_ref())
        .ok_or(BackendError::SchemaMismatch)?;
    if params.shard_number != 1
        || params.replication_factor != Some(1)
        || params.write_consistency_factor != Some(1)
        || params.sharding_method != Some(i32::from(ShardingMethod::Auto))
    {
        return Err(BackendError::SchemaMismatch);
    }
    if let Some(vectors) = &params.vectors_config
        && !matches!(&vectors.config, Some(vectors_config::Config::ParamsMap(map)) if map.map.is_empty())
    {
        return Err(BackendError::SchemaMismatch);
    }
    if params
        .sparse_vectors_config
        .as_ref()
        .is_some_and(|sparse| !sparse.map.is_empty())
        || info.payload_schema.len() != 1
    {
        return Err(BackendError::SchemaMismatch);
    }
    let text = info
        .payload_schema
        .get(TEXT_FIELD)
        .ok_or(BackendError::SchemaMismatch)?;
    if text.data_type != i32::from(PayloadSchemaType::Text) {
        return Err(BackendError::SchemaMismatch);
    }
    let Some(payload_index_params::IndexParams::TextIndexParams(params)) = text
        .params
        .as_ref()
        .and_then(|params| params.index_params.as_ref())
    else {
        return Err(BackendError::SchemaMismatch);
    };
    verify_text(params)
}

fn verify_text(params: &TextIndexParams) -> Result<(), BackendError> {
    if params.tokenizer != i32::from(TokenizerType::Code)
        || params.lowercase != Some(true)
        || params.ascii_folding != Some(false)
        || params.phrase_matching != Some(false)
        || params.enable_hnsw != Some(false)
        || params.memory != Some(i32::from(Memory::Pinned))
        || params.scoring.as_ref().map(|scoring| scoring.r#type)
            != Some(i32::from(TextScoringType::Bm25))
        || !matches!(
            params
                .stemmer
                .as_ref()
                .and_then(|stemmer| stemmer.stemming_params.as_ref()),
            Some(stemming_algorithm::StemmingParams::Disabled(_))
        )
        || params.stopwords.is_some()
        || params.min_token_len.is_some()
        || params.max_token_len.is_some()
        || params.nested.is_some()
    {
        return Err(BackendError::SchemaMismatch);
    }
    Ok(())
}

pub fn completed(status: i32) -> Result<(), BackendError> {
    match UpdateStatus::try_from(status) {
        Ok(UpdateStatus::Completed) => Ok(()),
        Ok(UpdateStatus::ClockRejected) => Err(BackendError::BackendRefused),
        _ => Err(BackendError::UnknownDispatch),
    }
}
