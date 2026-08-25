pub mod analytics;
pub use analytics::*;

use async_trait::async_trait;
use qs_core::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidationSummary {
    pub schema_version: u32,
    pub stage: String,
    pub candidates: Vec<ValidationCandidateSummary>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidationCandidateSummary {
    pub strategy_id: StrategyId,
    pub parameter_set_id: ParameterSetId,
    pub score: f64,
    pub notes: Vec<String>,
    pub metrics: BTreeMap<String, f64>,
}

fn candidate_score(result: &EvaluationResult) -> f64 {
    let c = &result.metrics.classical;
    let s = &result.metrics.stability;
    let pf_score = finite_or(c.profit_factor, 0.0).min(4.0) / 4.0;
    let dd_penalty = c.max_drawdown.min(1.0);
    let stability = match s.edge_stability_ratio {
        Some(value) => value,
        None => 0.0,
    };
    let trade_count_penalty = if result.trades.len() < 30 { 0.25 } else { 0.0 };
    (0.35 * c.expectancy.tanh()) + (0.25 * pf_score) + (0.30 * stability)
        - (0.30 * dd_penalty)
        - trade_count_penalty
}

fn finite_or(value: f64, fallback: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

fn build_summary(stage: &str, results: &[EvaluationResult]) -> ValidationSummary {
    let candidates = results
        .iter()
        .map(|result| {
            let mut metrics = BTreeMap::new();
            metrics.insert("total_r".into(), result.metrics.classical.total_r);
            metrics.insert(
                "profit_factor".into(),
                finite_or(result.metrics.classical.profit_factor, 0.0),
            );
            metrics.insert("max_drawdown".into(), result.metrics.classical.max_drawdown);
            if let Some(value) = result.metrics.stability.edge_stability_ratio {
                metrics.insert("edge_stability_ratio".into(), value);
            }
            ValidationCandidateSummary {
                strategy_id: result.strategy_id.clone(),
                parameter_set_id: result.parameter_set_id.clone(),
                score: candidate_score(result),
                notes: Vec::new(),
                metrics,
            }
        })
        .collect();
    ValidationSummary {
        schema_version: 1,
        stage: stage.into(),
        candidates,
    }
}

fn store_summary(ctx: &mut PipelineContext, stage: &str) -> Result<(), PipelineError> {
    let summary = build_summary(stage, &ctx.partial_results);
    let value = serde_json::to_value(summary).map_err(|e| {
        PipelineError::Validation(ValidationError::Message {
            code: "VALIDATION_SERIALIZE".into(),
            message: e.to_string(),
            retryable: false,
        })
    })?;
    ctx.bag.insert(format!("validation_{stage}"), value);
    Ok(())
}

fn store_stage_detail(ctx: &mut PipelineContext, stage: &str) -> Result<(), PipelineError> {
    match stage {
        "WalkForward" => {
            let reports = ctx
                .partial_results
                .iter()
                .map(|result| walk_forward_report(result, 5))
                .collect::<Vec<_>>();
            let value = serde_json::to_value(reports).map_err(|e| {
                PipelineError::Validation(ValidationError::Message {
                    code: "WALK_FORWARD_SERIALIZE".into(),
                    message: e.to_string(),
                    retryable: false,
                })
            })?;
            ctx.bag.insert("walk_forward_reports".into(), value);
        }
        "MonteCarlo" => {
            let seed = match ctx.run_config.as_ref().map(|config| config.seed) {
                Some(value) => value,
                None => 0,
            };
            let reports = ctx
                .partial_results
                .iter()
                .map(|result| monte_carlo_report(result, 250, seed))
                .collect::<Vec<_>>();
            let value = serde_json::to_value(reports).map_err(|e| {
                PipelineError::Validation(ValidationError::Message {
                    code: "MONTE_CARLO_SERIALIZE".into(),
                    message: e.to_string(),
                    retryable: false,
                })
            })?;
            ctx.bag.insert("monte_carlo_reports".into(), value);
        }
        "SensitivityAnalysis" => {
            let reports = ctx
                .partial_results
                .iter()
                .map(sensitivity_report)
                .collect::<Vec<_>>();
            let value = serde_json::to_value(reports).map_err(|e| {
                PipelineError::Validation(ValidationError::Message {
                    code: "SENSITIVITY_SERIALIZE".into(),
                    message: e.to_string(),
                    retryable: false,
                })
            })?;
            ctx.bag.insert("sensitivity_reports".into(), value);
        }
        "RegimeAnalysis" => {
            let reports = ctx
                .partial_results
                .iter()
                .map(regime_report)
                .collect::<Vec<_>>();
            let value = serde_json::to_value(reports).map_err(|e| {
                PipelineError::Validation(ValidationError::Message {
                    code: "REGIME_SERIALIZE".into(),
                    message: e.to_string(),
                    retryable: false,
                })
            })?;
            ctx.bag.insert("regime_reports".into(), value);
        }
        "ExecutionStress" => {
            let reports = ctx
                .partial_results
                .iter()
                .map(|result| execution_stress_report(result, 0.01))
                .collect::<Vec<_>>();
            let value = serde_json::to_value(reports).map_err(|e| {
                PipelineError::Validation(ValidationError::Message {
                    code: "STRESS_SERIALIZE".into(),
                    message: e.to_string(),
                    retryable: false,
                })
            })?;
            ctx.bag.insert("execution_stress_reports".into(), value);
        }
        "ParameterDecay" => {
            let reports = ctx
                .partial_results
                .iter()
                .map(parameter_decay_report)
                .collect::<Vec<_>>();
            let value = serde_json::to_value(reports).map_err(|e| {
                PipelineError::Validation(ValidationError::Message {
                    code: "DECAY_SERIALIZE".into(),
                    message: e.to_string(),
                    retryable: false,
                })
            })?;
            ctx.bag.insert("parameter_decay_reports".into(), value);
        }
        _ => {}
    }
    Ok(())
}

macro_rules! validation_component {
    ($name:ident, $id:literal) => {
        pub struct $name;
        #[async_trait]
        impl PipelineComponent for $name {
            fn id(&self) -> ComponentId {
                ComponentId($id.into())
            }
            fn name(&self) -> &'static str {
                $id
            }
            fn version(&self) -> ComponentVersion {
                ComponentVersion {
                    semver: "0.1.0".into(),
                }
            }
            fn input_contract(&self) -> Vec<DataContract> {
                vec![DataContract::EvaluationResults]
            }
            fn output_contract(&self) -> Vec<DataContract> {
                vec![DataContract::ValidationResults]
            }
            fn failure_policy(&self) -> FailurePolicy {
                FailurePolicy::AbortRun
            }
            async fn execute(
                &self,
                ctx: &mut PipelineContext,
            ) -> Result<ComponentOutcome, PipelineError> {
                store_summary(ctx, $id)?;
                store_stage_detail(ctx, $id)?;
                Ok(ComponentOutcome {
                    message: format!("{} validation summary completed", $id),
                })
            }
        }
    };
}

validation_component!(WalkForwardComponent, "WalkForward");
validation_component!(MonteCarloComponent, "MonteCarlo");
validation_component!(SensitivityAnalysisComponent, "SensitivityAnalysis");
validation_component!(RegimeAnalysisComponent, "RegimeAnalysis");
validation_component!(
    VarianceStabilityAnalysisComponent,
    "VarianceStabilityAnalysis"
);
validation_component!(ExecutionStressComponent, "ExecutionStress");
validation_component!(ParameterDecayComponent, "ParameterDecay");

pub struct FinalRankingComponent;
#[async_trait]
impl PipelineComponent for FinalRankingComponent {
    fn id(&self) -> ComponentId {
        ComponentId("FinalRanking".into())
    }
    fn name(&self) -> &'static str {
        "FinalRanking"
    }
    fn version(&self) -> ComponentVersion {
        ComponentVersion {
            semver: "0.1.0".into(),
        }
    }
    fn input_contract(&self) -> Vec<DataContract> {
        vec![DataContract::ValidationResults]
    }
    fn output_contract(&self) -> Vec<DataContract> {
        vec![DataContract::RankingResults]
    }
    fn failure_policy(&self) -> FailurePolicy {
        FailurePolicy::AbortRun
    }
    async fn execute(&self, ctx: &mut PipelineContext) -> Result<ComponentOutcome, PipelineError> {
        let mut ranked: Vec<_> = ctx
            .partial_results
            .iter()
            .map(|result| {
                serde_json::json!({
                    "strategy_id": result.strategy_id.0,
                    "parameter_set_id": result.parameter_set_id.0,
                    "score": candidate_score(result),
                    "total_r": result.metrics.classical.total_r,
                    "profit_factor": finite_or(result.metrics.classical.profit_factor, 0.0),
                    "max_drawdown": result.metrics.classical.max_drawdown,
                    "trade_count": result.trades.len()
                })
            })
            .collect();
        ranked.sort_by(|a, b| {
            let av = match a.get("score").and_then(|v| v.as_f64()) {
                Some(value) => value,
                None => f64::NEG_INFINITY,
            };
            let bv = match b.get("score").and_then(|v| v.as_f64()) {
                Some(value) => value,
                None => f64::NEG_INFINITY,
            };
            match bv.partial_cmp(&av) {
                Some(ordering) => ordering,
                None => std::cmp::Ordering::Equal,
            }
        });
        ctx.bag.insert(
            "ranking_state".into(),
            serde_json::json!({ "schema_version": 1, "ranked": ranked }),
        );
        Ok(ComponentOutcome {
            message: "final ranking completed".into(),
        })
    }
}
