use faer::col::Col;
use faer::prelude::SolveLstsq;
use faer::sparse::linalg::solvers::{Qr, SymbolicQr};
use faer::sparse::{SparseColMat, SymbolicSparseColMat};
use nalgebra::DVector;

use crate::SparseFallbackReason;
use crate::linearization::ComponentIndexedSystem;

pub(crate) const SPARSE_SYMBOLIC_CACHE_MAX_ENTRIES: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
struct SparsePatternKey {
    rows: usize,
    columns: usize,
    sparsity_signature: u64,
    exact_pattern: Vec<(usize, usize)>,
    free_columns: Vec<usize>,
}

#[derive(Clone, Debug)]
struct SparseSymbolicCacheEntry {
    key: SparsePatternKey,
    symbolic: SymbolicQr<usize>,
}

/// Entries are immutable after insertion and use deterministic FIFO eviction.
/// Cloning a problem clones this collection while faer's immutable symbolic
/// analysis remains reference-counted internally.
#[derive(Clone, Debug, Default)]
pub(crate) struct SparseSymbolicCache {
    entries: Vec<SparseSymbolicCacheEntry>,
}

#[derive(Debug)]
pub(crate) struct SparseSolveOutcome {
    pub(crate) step: DVector<f64>,
    pub(crate) symbolic_reused: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SparseSolveFailure {
    pub(crate) reason: SparseFallbackReason,
}

struct AugmentedCsc {
    matrix: SparseColMat<usize, f64>,
    key: SparsePatternKey,
}

pub(crate) fn restricted_structural_nnz(
    system: &ComponentIndexedSystem,
    free_columns: &[usize],
) -> Option<usize> {
    validate_free_columns(system.column_count, free_columns)?;
    let mut selected = vec![false; system.column_count];
    for &column in free_columns {
        selected[column] = true;
    }
    system.entries.iter().try_fold(0usize, |count, entry| {
        (entry.row < system.row_count && entry.column < system.column_count)
            .then(|| count + usize::from(selected[entry.column]))
    })
}

pub(crate) fn solve_damped_least_squares(
    system: &ComponentIndexedSystem,
    effective_residual: &DVector<f64>,
    free_columns: &[usize],
    damping: f64,
    normalized_step_tolerance: f64,
    cache: &mut SparseSymbolicCache,
) -> Result<SparseSolveOutcome, SparseSolveFailure> {
    let augmented = build_augmented_csc(system, effective_residual, free_columns, damping)
        .map_err(|reason| SparseSolveFailure { reason })?;
    let (symbolic, symbolic_reused) = if let Some(entry) = cache
        .entries
        .iter()
        .find(|entry| entry.key == augmented.key)
    {
        (entry.symbolic.clone(), true)
    } else {
        let symbolic =
            SymbolicQr::try_new(augmented.matrix.symbolic()).map_err(|_| SparseSolveFailure {
                reason: SparseFallbackReason::SymbolicAnalysisFailure,
            })?;
        if cache.entries.len() == SPARSE_SYMBOLIC_CACHE_MAX_ENTRIES {
            cache.entries.remove(0);
        }
        cache.entries.push(SparseSymbolicCacheEntry {
            key: augmented.key.clone(),
            symbolic: symbolic.clone(),
        });
        (symbolic, false)
    };
    let factor = Qr::try_new_with_symbolic(symbolic, augmented.matrix.as_ref()).map_err(|_| {
        SparseSolveFailure {
            reason: SparseFallbackReason::NumericFactorizationFailure,
        }
    })?;
    let rhs = Col::from_fn(system.row_count + free_columns.len(), |row| {
        if row < system.row_count {
            -effective_residual[row]
        } else {
            0.0
        }
    });
    let solution = factor.solve_lstsq(rhs);
    if solution.nrows() != free_columns.len() {
        return Err(SparseSolveFailure {
            reason: SparseFallbackReason::SolutionValidationFailure,
        });
    }
    let step = DVector::from_iterator(
        free_columns.len(),
        (0..free_columns.len()).map(|row| solution[row]),
    );
    validate_sparse_solution(
        system,
        effective_residual,
        free_columns,
        damping,
        normalized_step_tolerance,
        &step,
    )
    .ok_or(SparseSolveFailure {
        reason: SparseFallbackReason::SolutionValidationFailure,
    })?;
    Ok(SparseSolveOutcome {
        step,
        symbolic_reused,
    })
}

fn build_augmented_csc(
    system: &ComponentIndexedSystem,
    effective_residual: &DVector<f64>,
    free_columns: &[usize],
    damping: f64,
) -> Result<AugmentedCsc, SparseFallbackReason> {
    if effective_residual.len() != system.row_count
        || effective_residual.iter().any(|value| !value.is_finite())
        || !damping.is_finite()
        || damping <= 0.0
        || validate_free_columns(system.column_count, free_columns).is_none()
    {
        return Err(SparseFallbackReason::ConstructionFailure);
    }
    let sqrt_damping = damping.sqrt();
    if !sqrt_damping.is_finite() || sqrt_damping <= 0.0 {
        return Err(SparseFallbackReason::ConstructionFailure);
    }

    let mut reduced_columns = vec![None; system.column_count];
    for (reduced, &original) in free_columns.iter().enumerate() {
        reduced_columns[original] = Some(reduced);
    }
    let mut column_entries = vec![Vec::new(); free_columns.len()];
    let mut exact_pattern = Vec::with_capacity(system.entries.len());
    let mut previous = None;
    for entry in &system.entries {
        let position = (entry.row, entry.column);
        if entry.row >= system.row_count
            || entry.column >= system.column_count
            || !entry.value.is_finite()
            || previous.is_some_and(|previous| previous >= position)
        {
            return Err(SparseFallbackReason::ConstructionFailure);
        }
        previous = Some(position);
        exact_pattern.push(position);
        if let Some(reduced) = reduced_columns[entry.column] {
            column_entries[reduced].push((entry.row, entry.value));
        }
    }

    let augmented_rows = system
        .row_count
        .checked_add(free_columns.len())
        .ok_or(SparseFallbackReason::ConstructionFailure)?;
    let selected_nnz = column_entries.iter().try_fold(0usize, |count, entries| {
        count.checked_add(entries.len().saturating_add(1))
    });
    let selected_nnz = selected_nnz.ok_or(SparseFallbackReason::ConstructionFailure)?;
    let mut col_ptr = Vec::with_capacity(free_columns.len().saturating_add(1));
    let mut row_idx = Vec::with_capacity(selected_nnz);
    let mut values = Vec::with_capacity(selected_nnz);
    col_ptr.push(0);
    for (column, entries) in column_entries.into_iter().enumerate() {
        let mut previous_row = None;
        for (row, value) in entries {
            if previous_row.is_some_and(|previous| previous >= row) {
                return Err(SparseFallbackReason::ConstructionFailure);
            }
            previous_row = Some(row);
            row_idx.push(row);
            values.push(value);
        }
        row_idx.push(system.row_count + column);
        values.push(sqrt_damping);
        col_ptr.push(row_idx.len());
    }
    if row_idx.len() != selected_nnz || values.len() != selected_nnz {
        return Err(SparseFallbackReason::ConstructionFailure);
    }
    let symbolic = SymbolicSparseColMat::new_checked(
        augmented_rows,
        free_columns.len(),
        col_ptr,
        None,
        row_idx,
    );
    Ok(AugmentedCsc {
        matrix: SparseColMat::new(symbolic, values),
        key: SparsePatternKey {
            rows: system.row_count,
            columns: system.column_count,
            sparsity_signature: system.sparsity_signature,
            exact_pattern,
            free_columns: free_columns.to_vec(),
        },
    })
}

fn validate_free_columns(columns: usize, free_columns: &[usize]) -> Option<()> {
    let mut previous = None;
    for &column in free_columns {
        if column >= columns || previous.is_some_and(|previous| previous >= column) {
            return None;
        }
        previous = Some(column);
    }
    Some(())
}

#[allow(clippy::too_many_arguments)]
fn validate_sparse_solution(
    system: &ComponentIndexedSystem,
    effective_residual: &DVector<f64>,
    free_columns: &[usize],
    damping: f64,
    normalized_step_tolerance: f64,
    step: &DVector<f64>,
) -> Option<()> {
    if step.len() != free_columns.len()
        || step.iter().any(|value| !value.is_finite())
        || effective_residual.len() != system.row_count
        || !normalized_step_tolerance.is_finite()
        || normalized_step_tolerance <= 0.0
    {
        return None;
    }
    let mut reduced_columns = vec![None; system.column_count];
    for (reduced, &original) in free_columns.iter().enumerate() {
        reduced_columns[original] = Some(reduced);
    }
    let mut model_residual = effective_residual.clone();
    let mut jacobian_norm = 0.0_f64;
    for entry in &system.entries {
        if let Some(column) = reduced_columns[entry.column] {
            model_residual[entry.row] += entry.value * step[column];
            jacobian_norm = jacobian_norm.hypot(entry.value);
        }
    }
    let sqrt_damping = damping.sqrt();
    for _ in free_columns {
        jacobian_norm = jacobian_norm.hypot(sqrt_damping);
    }
    let residual_norm = stable_norm(effective_residual.iter().copied())?;
    let model_norm = stable_norm(model_residual.iter().copied())?;
    let step_norm = stable_norm(step.iter().copied())?;
    let augmented_norm = model_norm.hypot(sqrt_damping * step_norm);
    let residual_roundoff =
        256.0 * f64::EPSILON * (jacobian_norm * step_norm + residual_norm).max(residual_norm);
    if !augmented_norm.is_finite()
        || !residual_roundoff.is_finite()
        || augmented_norm > residual_norm + residual_roundoff
    {
        return None;
    }

    let mut normal_residual = DVector::from_element(free_columns.len(), 0.0);
    for entry in &system.entries {
        if let Some(column) = reduced_columns[entry.column] {
            normal_residual[column] += entry.value * model_residual[entry.row];
        }
    }
    normal_residual += step * damping;
    let normal_norm = stable_norm(normal_residual.iter().copied())?;
    let normal_scale = jacobian_norm * (jacobian_norm * step_norm + residual_norm);
    let normal_tolerance = (256.0 * f64::EPSILON * normal_scale)
        .max(normalized_step_tolerance * jacobian_norm * jacobian_norm);
    if !normal_tolerance.is_finite() || normal_norm > normal_tolerance {
        return None;
    }

    let baseline_cost = 0.5 * effective_residual.dot(effective_residual);
    let model_cost = 0.5 * model_residual.dot(&model_residual);
    let predicted_reduction = baseline_cost - model_cost;
    let prediction_tolerance = 256.0 * f64::EPSILON * baseline_cost.abs().max(model_cost.abs());
    (baseline_cost.is_finite()
        && model_cost.is_finite()
        && predicted_reduction.is_finite()
        && prediction_tolerance.is_finite()
        && predicted_reduction >= -prediction_tolerance)
        .then_some(())
}

fn stable_norm(values: impl Iterator<Item = f64>) -> Option<f64> {
    let mut norm = 0.0_f64;
    for value in values {
        if !value.is_finite() {
            return None;
        }
        norm = norm.hypot(value);
        if !norm.is_finite() {
            return None;
        }
    }
    Some(norm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linearization::{IndexedJacobianEntry, RowIdentity};

    fn system() -> ComponentIndexedSystem {
        ComponentIndexedSystem {
            residuals: DVector::from_vec(vec![-1.0, -2.0]),
            rows: vec![
                RowIdentity {
                    residual_id: crate::ResidualId::default(),
                    source_id: crate::SourceConstraintId::default(),
                    row_in_block: 0,
                },
                RowIdentity {
                    residual_id: crate::ResidualId::default(),
                    source_id: crate::SourceConstraintId::default(),
                    row_in_block: 1,
                },
            ],
            row_count: 2,
            column_count: 2,
            entries: vec![
                IndexedJacobianEntry {
                    row: 0,
                    column: 0,
                    value: 1.0,
                },
                IndexedJacobianEntry {
                    row: 1,
                    column: 1,
                    value: 1.0,
                },
            ],
            sparsity_signature: 7,
        }
    }

    #[test]
    fn exact_pattern_and_free_mask_control_symbolic_reuse() {
        let system = system();
        let mut cache = SparseSymbolicCache::default();
        let first = solve_damped_least_squares(
            &system,
            &system.residuals,
            &[0, 1],
            1.0e-6,
            1.0e-10,
            &mut cache,
        )
        .unwrap();
        assert!(!first.symbolic_reused);
        let second = solve_damped_least_squares(
            &system,
            &system.residuals,
            &[0, 1],
            1.0e-3,
            1.0e-10,
            &mut cache,
        )
        .unwrap();
        assert!(second.symbolic_reused);
        let subset = solve_damped_least_squares(
            &system,
            &DVector::from_vec(vec![-1.0, -2.0]),
            &[1],
            1.0e-3,
            1.0e-10,
            &mut cache,
        )
        .unwrap();
        assert!(!subset.symbolic_reused);
        assert_eq!(cache.entries.len(), 2);
    }

    #[test]
    fn symbolic_cache_has_deterministic_fifo_capacity() {
        let mut cache = SparseSymbolicCache::default();
        for signature in 0..=SPARSE_SYMBOLIC_CACHE_MAX_ENTRIES {
            let mut candidate = system();
            candidate.sparsity_signature = u64::try_from(signature).unwrap();
            let outcome = solve_damped_least_squares(
                &candidate,
                &candidate.residuals,
                &[0, 1],
                1.0e-6,
                1.0e-10,
                &mut cache,
            )
            .unwrap();
            assert!(!outcome.symbolic_reused);
            assert!(cache.entries.len() <= SPARSE_SYMBOLIC_CACHE_MAX_ENTRIES);
        }
        assert_eq!(cache.entries.len(), SPARSE_SYMBOLIC_CACHE_MAX_ENTRIES);

        let mut evicted = system();
        evicted.sparsity_signature = 0;
        let outcome = solve_damped_least_squares(
            &evicted,
            &evicted.residuals,
            &[0, 1],
            1.0e-6,
            1.0e-10,
            &mut cache,
        )
        .unwrap();
        assert!(!outcome.symbolic_reused);
        assert_eq!(cache.entries.len(), SPARSE_SYMBOLIC_CACHE_MAX_ENTRIES);
    }

    #[test]
    fn malformed_indexed_system_is_classified_as_construction_fallback() {
        let mut malformed = system();
        malformed.entries.swap(0, 1);
        let failure = solve_damped_least_squares(
            &malformed,
            &malformed.residuals,
            &[0, 1],
            1.0e-6,
            1.0e-10,
            &mut SparseSymbolicCache::default(),
        )
        .unwrap_err();
        assert_eq!(failure.reason, SparseFallbackReason::ConstructionFailure);
    }
}
