use crate::ParameterNeighborhood;
use qs_core::*;
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct GenerationBudget {
    pub max_candidates: usize,
}

pub fn generate_grid(
    space: &ParameterSpace,
    budget: GenerationBudget,
) -> Result<Vec<ParameterSet>, SearchError> {
    let mut partials: Vec<BTreeMap<String, ParameterValue>> = vec![BTreeMap::new()];
    for def in &space.parameters {
        let values = values_for_definition(def)?;
        let mut next = Vec::new();
        for existing in &partials {
            for value in &values {
                let mut map = existing.clone();
                map.insert(def.name.clone(), value.clone());
                next.push(map);
                if next.len() >= budget.max_candidates {
                    break;
                }
            }
            if next.len() >= budget.max_candidates {
                break;
            }
        }
        partials = next;
        if partials.len() >= budget.max_candidates {
            break;
        }
    }
    Ok(partials
        .into_iter()
        .enumerate()
        .map(|(idx, values)| ParameterSet {
            id: ParameterSetId(format!("{}_grid_{idx}", space.strategy_id.0)),
            strategy_id: space.strategy_id.clone(),
            values,
            source: ParameterSetSource::Grid,
            parent_ids: Vec::new(),
        })
        .collect())
}

fn values_for_definition(def: &ParameterDefinition) -> Result<Vec<ParameterValue>, SearchError> {
    match &def.kind {
        ParameterKind::IntRange { min, max, step } => {
            if *step <= 0 {
                return Err(search_error(
                    "INVALID_INT_STEP",
                    format!("{} step must be positive", def.name),
                ));
            }
            let mut out = Vec::new();
            let mut value = *min;
            while value <= *max {
                out.push(ParameterValue::Int(value));
                value += step;
            }
            Ok(out)
        }
        ParameterKind::FloatRange { min, max, step, .. } => {
            let Some(step) = step else {
                return Ok(vec![
                    ParameterValue::Float(*min),
                    ParameterValue::Float((*min + *max) / 2.0),
                    ParameterValue::Float(*max),
                ]);
            };
            if *step <= 0.0 {
                return Err(search_error(
                    "INVALID_FLOAT_STEP",
                    format!("{} step must be positive", def.name),
                ));
            }
            let mut out = Vec::new();
            let mut value = *min;
            while value <= *max {
                out.push(ParameterValue::Float(value));
                value += step;
            }
            Ok(out)
        }
        ParameterKind::Bool => Ok(vec![
            ParameterValue::Bool(false),
            ParameterValue::Bool(true),
        ]),
        ParameterKind::Enum { values } => {
            Ok(values.iter().cloned().map(ParameterValue::Enum).collect())
        }
    }
}

fn search_error(code: &str, message: String) -> SearchError {
    SearchError::Message {
        code: code.into(),
        message,
        retryable: false,
    }
}

pub struct DefaultNeighborhood;
impl ParameterNeighborhood for DefaultNeighborhood {
    fn distance(
        &self,
        a: &ParameterSet,
        b: &ParameterSet,
        space: &ParameterSpace,
    ) -> Result<f64, SearchError> {
        let mut sum = 0.0;
        for def in &space.parameters {
            let weight = match space.neighborhood.weights.get(&def.name).copied() {
                Some(value) => value,
                None => 1.0,
            };
            let av = a.values.get(&def.name);
            let bv = b.values.get(&def.name);
            sum += weight * parameter_distance(av, bv, &def.kind)?;
        }
        Ok(sum.sqrt())
    }

    fn neighbors(
        &self,
        center: &ParameterSet,
        radius: f64,
        budget: usize,
        space: &ParameterSpace,
    ) -> Result<Vec<ParameterSet>, SearchError> {
        let candidates = generate_grid(
            space,
            GenerationBudget {
                max_candidates: budget.saturating_mul(20).max(budget),
            },
        )?;
        let mut out = Vec::new();
        for candidate in candidates {
            if candidate.id == center.id {
                continue;
            }
            if self.distance(center, &candidate, space)? <= radius {
                out.push(ParameterSet {
                    source: ParameterSetSource::NeighborhoodExpansion,
                    parent_ids: vec![center.id.clone()],
                    ..candidate
                });
                if out.len() >= budget {
                    break;
                }
            }
        }
        Ok(out)
    }
}

fn parameter_distance(
    a: Option<&ParameterValue>,
    b: Option<&ParameterValue>,
    kind: &ParameterKind,
) -> Result<f64, SearchError> {
    let Some(a) = a else {
        return Ok(1.0);
    };
    let Some(b) = b else {
        return Ok(1.0);
    };
    match (a, b, kind) {
        (
            ParameterValue::Int(x),
            ParameterValue::Int(y),
            ParameterKind::IntRange { min, max, .. },
        ) => {
            let denom = (*max - *min).abs().max(1) as f64;
            Ok((*x - *y).abs() as f64 / denom)
        }
        (
            ParameterValue::Float(x),
            ParameterValue::Float(y),
            ParameterKind::FloatRange { min, max, .. },
        ) => {
            let denom = (*max - *min).abs().max(f64::EPSILON);
            Ok((*x - *y).abs() / denom)
        }
        (ParameterValue::Bool(x), ParameterValue::Bool(y), _) => Ok(if x == y { 0.0 } else { 1.0 }),
        (ParameterValue::Enum(x), ParameterValue::Enum(y), _) => Ok(if x == y { 0.0 } else { 1.0 }),
        _ => Ok(1.0),
    }
}

pub fn generate_budgeted(
    space: &ParameterSpace,
    budget: GenerationBudget,
    seed: u64,
) -> Result<Vec<ParameterSet>, SearchError> {
    let per_dimension: Vec<Vec<ParameterValue>> = space
        .parameters
        .iter()
        .map(values_for_definition)
        .collect::<Result<_, _>>()?;
    let total = per_dimension.iter().try_fold(1usize, |acc, values| {
        acc.checked_mul(values.len()).ok_or_else(|| {
            search_error(
                "SPACE_TOO_LARGE",
                "parameter space cardinality overflow".into(),
            )
        })
    })?;
    if total <= budget.max_candidates {
        return generate_grid(space, budget);
    }
    generate_sparse(space, &per_dimension, budget.max_candidates, seed)
}

fn generate_sparse(
    space: &ParameterSpace,
    per_dimension: &[Vec<ParameterValue>],
    budget: usize,
    seed: u64,
) -> Result<Vec<ParameterSet>, SearchError> {
    if per_dimension.iter().any(Vec::is_empty) {
        return Ok(Vec::new());
    }
    let mut state = if seed == 0 {
        0x9E37_79B9_7F4A_7C15
    } else {
        seed
    };
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    let max_attempts = budget.saturating_mul(20).max(100);
    for _ in 0..max_attempts {
        if out.len() >= budget {
            break;
        }
        let mut values = BTreeMap::new();
        let mut key_parts = Vec::new();
        for (def, dimension) in space.parameters.iter().zip(per_dimension.iter()) {
            state = lcg_next(state);
            let idx = (state as usize) % dimension.len();
            let value = dimension[idx].clone();
            key_parts.push(format!("{}={:?}", def.name, value));
            values.insert(def.name.clone(), value);
        }
        let key = key_parts.join("|");
        if seen.insert(key) {
            let id = ParameterSetId(format!("{}_sparse_{}", space.strategy_id.0, out.len()));
            out.push(ParameterSet {
                id,
                strategy_id: space.strategy_id.clone(),
                values,
                source: ParameterSetSource::RandomSparse,
                parent_ids: Vec::new(),
            });
        }
    }
    Ok(out)
}

fn lcg_next(state: u64) -> u64 {
    state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407)
}
