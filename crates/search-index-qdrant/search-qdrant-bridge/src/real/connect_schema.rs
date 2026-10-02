// Real adapter connection and collection-schema operations.

use super::{
    BTreeMap, BridgeError, BridgeLimits, CollectionRoute, CollectionSchema, CreateCollection,
    CreateFieldIndexCollection, Duration, FieldType, HashMap, LiveEndpoint, Modifier,
    OpContext, OperationBudget, PayloadSchemaType, PointPayload, Qdrant, QualifiedGate,
    ReceiptRef, RealDataPlane, SparseVectorConfig, SparseVectorParams, StrictModeConfig,
    UpdateCollection, QUALIFIED_SERVER_BUILD, QUALIFIED_SERVER_VERSION, collection_name,
    map_create_error, map_read_error, strong_ordering, update_completed, verify_server_schema,
};

include!("connect_schema/connect.rs");
include!("connect_schema/create.rs");
include!("connect_schema/verify.rs");
