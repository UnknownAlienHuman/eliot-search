//! Closed recipe-handler registry for the canonical admitted query host.
//!
//! This module owns no recipe semantics, planner, source read, provider call or
//! result projection. Package-owned handlers register exactly one `RecipeIdV1`;
//! the registry performs deterministic dispatch and guarantees that every task
//! receives canonical pre-retrieval admission before any authorized work.

use std::collections::{BTreeMap, BTreeSet};
use std::task::Poll;

use search_contracts::RecipeIdV1;
use search_provider_protocol::{AdmittedProviderRequest, BindingContext};

use crate::access_composition::StandalonePreRetrievalAdmission;
use crate::provider_composition::{
    CanonicalServingError, CanonicalWorkBudget, CanonicalWorkOutput,
};
use crate::query_serving_composition::{
    CanonicalAdmittedRecipeFactory, CanonicalAdmittedRecipeTask,
};

const MAX_REGISTERED_RECIPES: usize = RecipeIdV1::ALL.len();
const HANDLER_UNAVAILABLE: &str = "DAEMON_RECIPE_HANDLER_UNAVAILABLE";

/// Package-owned constructor for one exact admitted recipe.
///
/// `prepare` may retain only bounded request-local state. It must not perform a
/// source read, provider call, IDF/count operation, durable mutation or result
/// emission. Those effects are permitted only after the returned task receives
/// [`StandalonePreRetrievalAdmission`] through `poll_authorized`.
pub(crate) trait RegisteredAdmittedRecipeHandler {
    /// Exact recipe handled by this constructor.
    fn recipe_id(&self) -> RecipeIdV1;

    /// Construct one owned request-local task without performing recipe work.
    fn prepare(
        &mut self,
        binding: &BindingContext,
        request: &AdmittedProviderRequest,
        budget: CanonicalWorkBudget,
    ) -> Result<Box<dyn CanonicalAdmittedRecipeTask>, CanonicalServingError>;
}

/// Invalid closed handler population.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CanonicalRecipeRegistryError {
    /// A query host with no live recipe handlers cannot be published.
    Empty,
    /// More handlers were supplied than the closed P00 recipe vocabulary.
    TooMany,
    /// Two handlers claimed the same recipe identity.
    Duplicate(RecipeIdV1),
}

impl std::fmt::Display for CanonicalRecipeRegistryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => formatter.write_str("DAEMON_RECIPE_REGISTRY_EMPTY"),
            Self::TooMany => formatter.write_str("DAEMON_RECIPE_REGISTRY_TOO_LARGE"),
            Self::Duplicate(_) => formatter.write_str("DAEMON_RECIPE_HANDLER_DUPLICATE"),
        }
    }
}

impl std::error::Error for CanonicalRecipeRegistryError {}

/// Deterministic one-handler-per-recipe factory used by [`CanonicalQueryHost`].
///
/// The registered set is the sole source for capability publication. There is no
/// default handler, recipe alias, natural-language fallback or first-match order.
pub(crate) struct CanonicalRecipeRegistry {
    handlers: BTreeMap<RecipeIdV1, Box<dyn RegisteredAdmittedRecipeHandler>>,
}

impl CanonicalRecipeRegistry {
    /// Validate and retain one finite closed handler population.
    pub(crate) fn new(
        handlers: Vec<Box<dyn RegisteredAdmittedRecipeHandler>>,
    ) -> Result<Self, CanonicalRecipeRegistryError> {
        if handlers.is_empty() {
            return Err(CanonicalRecipeRegistryError::Empty);
        }
        if handlers.len() > MAX_REGISTERED_RECIPES {
            return Err(CanonicalRecipeRegistryError::TooMany);
        }
        let mut registered = BTreeMap::new();
        for handler in handlers {
            let recipe_id = handler.recipe_id();
            if registered.insert(recipe_id, handler).is_some() {
                return Err(CanonicalRecipeRegistryError::Duplicate(recipe_id));
            }
        }
        Ok(Self { handlers: registered })
    }

    /// Exact recipe set that may be advertised by the enclosing capability.
    #[must_use]
    pub(crate) fn registered_recipes(&self) -> BTreeSet<RecipeIdV1> {
        self.handlers.keys().copied().collect()
    }

    /// Whether one exact recipe has a live constructor.
    #[must_use]
    pub(crate) fn contains(&self, recipe_id: RecipeIdV1) -> bool {
        self.handlers.contains_key(&recipe_id)
    }
}

/// One task selected by exact recipe identity and owning one package task.
pub(crate) struct RegisteredAdmittedRecipeTask {
    recipe_id: RecipeIdV1,
    inner: Box<dyn CanonicalAdmittedRecipeTask>,
}

impl CanonicalAdmittedRecipeFactory for CanonicalRecipeRegistry {
    type Task = RegisteredAdmittedRecipeTask;

    fn prepare(
        &mut self,
        binding: &BindingContext,
        request: &AdmittedProviderRequest,
        budget: CanonicalWorkBudget,
    ) -> Result<Self::Task, CanonicalServingError> {
        budget.check()?;
        let recipe_id = request.body().recipe_request.recipe;
        let handler = self
            .handlers
            .get_mut(&recipe_id)
            .ok_or(CanonicalServingError::Backend(HANDLER_UNAVAILABLE))?;
        if handler.recipe_id() != recipe_id {
            return Err(CanonicalServingError::InvalidConfiguration);
        }
        let inner = handler.prepare(binding, request, budget)?;
        budget.check()?;
        Ok(RegisteredAdmittedRecipeTask { recipe_id, inner })
    }
}

impl CanonicalAdmittedRecipeTask for RegisteredAdmittedRecipeTask {
    fn poll_authorized(
        &mut self,
        admission: &StandalonePreRetrievalAdmission,
        output: &mut CanonicalWorkOutput<'_, '_>,
        budget: CanonicalWorkBudget,
    ) -> Result<Poll<()>, CanonicalServingError> {
        budget.check()?;
        let result = self.inner.poll_authorized(admission, output, budget);
        if result.is_err() {
            self.inner.abort();
        }
        result
    }

    fn poll_cancel(
        &mut self,
        budget: CanonicalWorkBudget,
    ) -> Result<Poll<()>, CanonicalServingError> {
        self.inner.poll_cancel(budget)
    }

    fn abort(&mut self) {
        self.inner.abort();
    }
}

impl std::fmt::Debug for RegisteredAdmittedRecipeTask {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RegisteredAdmittedRecipeTask")
            .field("recipe_id", &self.recipe_id)
            .finish_non_exhaustive()
    }
}
