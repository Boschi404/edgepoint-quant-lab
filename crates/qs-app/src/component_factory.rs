use qs_core::{DataQualityPolicy, PipelineComponent};
use qs_strategy_api::StaticStrategyRegistry;

pub fn build_static_strategy_registry() -> Result<StaticStrategyRegistry, qs_core::StrategyError> {
    let mut registry = StaticStrategyRegistry::new();
    registry.register(qs_example_strategy::MovingAverageToyStrategy)?;
    Ok(registry)
}

pub fn build_default_components(
    registry: StaticStrategyRegistry,
) -> Vec<Box<dyn PipelineComponent>> {
    vec![
        Box::new(qs_data::DataIngestionComponent),
        Box::new(qs_data::DataNormalizerComponent),
        Box::new(qs_data::DataQualityGateComponent {
            policy: DataQualityPolicy::Block,
        }),
        Box::new(qs_search::ParameterGeneratorComponent),
        Box::new(qs_search::ParameterSearchComponent),
        Box::new(qs_backtest::StrategyRunnerComponent::new(
            registry,
            qs_backtest::ExecutionModel::default(),
        )),
        Box::new(qs_validation::WalkForwardComponent),
        Box::new(qs_validation::MonteCarloComponent),
        Box::new(qs_validation::SensitivityAnalysisComponent),
        Box::new(qs_validation::RegimeAnalysisComponent),
        Box::new(qs_validation::VarianceStabilityAnalysisComponent),
        Box::new(qs_validation::ExecutionStressComponent),
        Box::new(qs_validation::ParameterDecayComponent),
        Box::new(qs_validation::FinalRankingComponent),
        Box::new(qs_export::LiveExportComponent),
        Box::new(qs_export::ReportGeneratorComponent),
    ]
}
