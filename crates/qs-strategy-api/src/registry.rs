use crate::StrategyPlugin;
use qs_core::*;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Default)]
pub struct StaticStrategyRegistry {
    strategies: BTreeMap<StrategyId, Arc<dyn StrategyPlugin>>,
}

impl StaticStrategyRegistry {
    pub fn new() -> Self {
        Self {
            strategies: BTreeMap::new(),
        }
    }

    pub fn register<P>(&mut self, plugin: P) -> Result<(), StrategyError>
    where
        P: StrategyPlugin + 'static,
    {
        let metadata = plugin.metadata();
        if self.strategies.contains_key(&metadata.strategy_id) {
            return Err(StrategyError::Message {
                code: "STRATEGY_DUPLICATE".into(),
                message: format!("strategy {} already registered", metadata.strategy_id.0),
                retryable: false,
            });
        }
        self.strategies
            .insert(metadata.strategy_id, Arc::new(plugin));
        Ok(())
    }

    pub fn get(&self, id: &StrategyId) -> Option<Arc<dyn StrategyPlugin>> {
        self.strategies.get(id).cloned()
    }

    pub fn list_ids(&self) -> Vec<StrategyId> {
        self.strategies.keys().cloned().collect()
    }

    pub fn parameter_spaces(&self) -> BTreeMap<StrategyId, ParameterSpace> {
        self.strategies
            .iter()
            .map(|(id, plugin)| (id.clone(), plugin.parameter_space()))
            .collect()
    }
}
