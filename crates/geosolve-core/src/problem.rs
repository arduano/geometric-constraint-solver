use std::ops::Range;

use nalgebra::{DMatrix, DVector};
use slotmap::{Key, SlotMap};

use crate::{
    AuditBinding, BoundId, BoundStatus, CoordinateBound, CoreError, DiagnosticCompleteness,
    EvaluationErrorCategory, ResidualBlock, ResidualCategory, ResidualId, SourceConstraint,
    SourceConstraintId, VariableBlock, VariableId, VariableKind, VariableValue,
    analysis::{AliasElimination, DecompositionCache, FixedElimination},
    linearization::{evaluate_values, normalize_residuals},
};

#[derive(Clone, Debug)]
pub(crate) struct StableStore<K: Key, V> {
    pub(crate) values: SlotMap<K, V>,
    pub(crate) insertion_order: Vec<K>,
}

impl<K: Key, V> StableStore<K, V> {
    fn new() -> Self {
        Self {
            values: SlotMap::with_key(),
            insertion_order: Vec::new(),
        }
    }

    fn insert(&mut self, value: V) -> K {
        let key = self.values.insert(value);
        self.insertion_order.push(key);
        key
    }

    pub(crate) fn get(&self, key: K) -> Option<&V> {
        self.values.get(key)
    }

    pub(crate) fn get_mut(&mut self, key: K) -> Option<&mut V> {
        self.values.get_mut(key)
    }

    fn remove(&mut self, key: K) -> Option<V> {
        self.values.remove(key)
    }

    pub(crate) fn replace(&mut self, key: K, value: V) -> Option<V> {
        self.values
            .get_mut(key)
            .map(|current| std::mem::replace(current, value))
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (K, &V)> {
        self.insertion_order
            .iter()
            .filter_map(|&key| self.values.get(key).map(|value| (key, value)))
    }
}

/// The deterministic ambient and normalized-tangent ranges for one variable.
#[derive(Clone, Debug, PartialEq)]
pub struct BlockLayout {
    pub variable_id: VariableId,
    pub kind: VariableKind,
    pub ambient_range: Range<usize>,
    pub tangent_range: Range<usize>,
    pub step_scales: Vec<f64>,
}

/// A deterministic packed variable layout in stable insertion order.
#[derive(Clone, Debug, PartialEq)]
pub struct PackedLayout {
    blocks: Vec<BlockLayout>,
    ambient_dimension: usize,
    tangent_dimension: usize,
}

impl PackedLayout {
    #[must_use]
    pub fn blocks(&self) -> &[BlockLayout] {
        &self.blocks
    }

    #[must_use]
    pub const fn ambient_dimension(&self) -> usize {
        self.ambient_dimension
    }

    #[must_use]
    pub const fn tangent_dimension(&self) -> usize {
        self.tangent_dimension
    }

    #[must_use]
    pub fn block(&self, variable_id: VariableId) -> Option<&BlockLayout> {
        self.blocks
            .iter()
            .find(|block| block.variable_id == variable_id)
    }
}

/// Current ambient values paired with their deterministic layout.
#[derive(Clone, Debug, PartialEq)]
pub struct PackedState {
    layout: PackedLayout,
    ambient: DVector<f64>,
}

impl PackedState {
    #[must_use]
    pub const fn layout(&self) -> &PackedLayout {
        &self.layout
    }

    #[must_use]
    pub const fn ambient(&self) -> &DVector<f64> {
        &self.ambient
    }
}

/// The deterministic packed row range of one residual block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResidualLayout {
    pub residual_id: ResidualId,
    pub row_range: Range<usize>,
}

/// Dimensionless dense residual and Jacobian data.
///
/// Jacobian columns differentiate with respect to normalized tangent
/// coordinates: `local_delta = step_scale * normalized_delta`.
#[derive(Clone, Debug, PartialEq)]
pub struct DenseAssembly {
    variable_layout: PackedLayout,
    residual_layout: Vec<ResidualLayout>,
    residuals: DVector<f64>,
    jacobian: DMatrix<f64>,
}

impl DenseAssembly {
    #[must_use]
    pub const fn variable_layout(&self) -> &PackedLayout {
        &self.variable_layout
    }

    #[must_use]
    pub fn residual_layout(&self) -> &[ResidualLayout] {
        &self.residual_layout
    }

    #[must_use]
    pub const fn residuals(&self) -> &DVector<f64> {
        &self.residuals
    }

    #[must_use]
    pub const fn jacobian(&self) -> &DMatrix<f64> {
        &self.jacobian
    }

    #[must_use]
    pub fn residual_range(&self, residual_id: ResidualId) -> Option<Range<usize>> {
        self.residual_layout
            .iter()
            .find(|block| block.residual_id == residual_id)
            .map(|block| block.row_range.clone())
    }
}

/// Complete static audit metadata for one executable scalar row.
#[derive(Clone, Debug, PartialEq)]
pub struct AuditRowDescriptor {
    pub residual_id: ResidualId,
    pub source_id: SourceConstraintId,
    pub source_label: String,
    pub category: ResidualCategory,
    pub row_in_block: usize,
    pub template: String,
    pub bindings: Vec<AuditBinding>,
    pub unit: String,
    pub scale: f64,
}

/// One evaluated scalar row from a particular problem state.
#[derive(Clone, Debug, PartialEq)]
pub struct AuditRowSnapshot {
    pub residual_id: ResidualId,
    pub category: ResidualCategory,
    pub row_in_block: usize,
    pub template: String,
    /// Static feature/reference bindings copied from the row descriptor.
    pub bindings: Vec<AuditBinding>,
    /// Current values of incident variables in declared incidence order.
    pub incident_variables: Vec<AuditVariableSnapshot>,
    pub unit: String,
    pub scale: f64,
    pub raw_residual: f64,
    pub normalized_residual: f64,
    /// Whether both value and canonical Jacobian/fused evaluation succeeded.
    pub evaluation_status: AuditEvaluationStatus,
    /// Machine-readable semantic category retained from evaluator failure.
    pub evaluation_error_category: Option<EvaluationErrorCategory>,
    /// Human-readable evaluation failure, when the row was not evaluated completely.
    pub evaluation_error: Option<String>,
    pub annotations: AuditAnnotations,
    /// Exact active/fixed bound identities affecting incident coordinates.
    pub active_bounds: Vec<AuditBoundAnnotation>,
}

/// Whether executable value/Jacobian evaluation succeeded for an audit row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AuditEvaluationStatus {
    /// Fresh value and canonical Jacobian/fused evaluation both succeeded.
    Evaluated,
    /// At least one required evaluation failed; any available fresh values are retained.
    Failed,
}

/// One incident variable evaluated at the audit snapshot state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuditVariableSnapshot {
    pub variable_id: VariableId,
    pub value: VariableValue,
}

/// Evaluated rows grouped under one high-level source constraint.
#[derive(Clone, Debug, PartialEq)]
pub struct AuditSourceSnapshot {
    pub source_id: SourceConstraintId,
    pub source_label: String,
    pub rows: Vec<AuditRowSnapshot>,
    pub annotations: AuditAnnotations,
    /// Deduplicated active/fixed bounds affecting any source row.
    pub active_bounds: Vec<AuditBoundAnnotation>,
}

/// Structured accepted-state bound link attached to equation audit data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuditBoundAnnotation {
    pub bound_id: BoundId,
    pub variable_id: VariableId,
    pub coordinate: usize,
    pub status: BoundStatus,
}

/// Diagnostic flags evaluated at the same returned state as audit values.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct AuditAnnotations {
    pub eliminated: bool,
    pub suppressed: bool,
    pub redundant: bool,
    pub conflicting: bool,
    pub singular: bool,
    /// At least one incident coordinate has an active or fixed bound.
    pub active_bound: bool,
    /// Completeness of the candidate algorithm behind `redundant`.
    pub redundancy_diagnostics: Option<DiagnosticCompleteness>,
    /// Completeness of the candidate algorithm behind `conflicting`.
    pub conflict_diagnostics: Option<DiagnosticCompleteness>,
}

/// Deterministically ordered equation audit for a finite problem state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AuditSnapshot {
    pub sources: Vec<AuditSourceSnapshot>,
}

/// Analytic-versus-central-difference errors for one incidence block.
#[derive(Clone, Debug, PartialEq)]
pub struct JacobianBlockReport {
    pub residual_id: ResidualId,
    pub variable_id: VariableId,
    pub rows: usize,
    pub columns: usize,
    pub max_absolute_error: f64,
    pub max_relative_error: f64,
    pub worst_row: usize,
    pub worst_column: usize,
}

/// Per-incidence reports from central finite-difference verification.
#[derive(Clone, Debug, PartialEq)]
pub struct JacobianCheckReport {
    pub normalized_step: f64,
    pub blocks: Vec<JacobianBlockReport>,
}

impl JacobianCheckReport {
    #[must_use]
    pub fn max_absolute_error(&self) -> f64 {
        self.blocks
            .iter()
            .map(|block| block.max_absolute_error)
            .fold(0.0, f64::max)
    }

    #[must_use]
    pub fn max_relative_error(&self) -> f64 {
        self.blocks
            .iter()
            .map(|block| block.max_relative_error)
            .fold(0.0, f64::max)
    }

    #[must_use]
    pub fn all_within(&self, tolerance: f64) -> bool {
        tolerance.is_finite()
            && tolerance >= 0.0
            && self
                .blocks
                .iter()
                .all(|block| block.max_relative_error <= tolerance)
    }
}

/// Domain-independent storage and assembly for one equality problem.
#[derive(Clone, Debug)]
pub struct Problem {
    pub(crate) variables: StableStore<VariableId, VariableBlock>,
    pub(crate) residuals: StableStore<ResidualId, ResidualBlock>,
    pub(crate) sources: StableStore<SourceConstraintId, SourceConstraint>,
    pub(crate) bounds: StableStore<BoundId, CoordinateBound>,
    pub(crate) fixed_eliminations: Vec<FixedElimination>,
    pub(crate) alias_eliminations: Vec<AliasElimination>,
    pub(crate) decomposition_cache: Option<DecompositionCache>,
}

#[derive(Clone, Debug)]
pub(crate) struct VariableState {
    pub(crate) values: Vec<(VariableId, VariableValue)>,
}

impl Default for Problem {
    fn default() -> Self {
        Self::new()
    }
}

impl Problem {
    #[must_use]
    pub fn new() -> Self {
        Self {
            variables: StableStore::new(),
            residuals: StableStore::new(),
            sources: StableStore::new(),
            bounds: StableStore::new(),
            fixed_eliminations: Vec::new(),
            alias_eliminations: Vec::new(),
            decomposition_cache: None,
        }
    }

    pub fn add_variable(&mut self, variable: VariableBlock) -> VariableId {
        self.variables.insert(variable)
    }

    /// Removes an unreferenced variable block.
    ///
    /// # Errors
    ///
    /// Returns an error if the ID is stale or a residual still references it.
    pub fn remove_variable(&mut self, variable_id: VariableId) -> Result<VariableBlock, CoreError> {
        if self
            .residuals
            .iter()
            .any(|(_, residual)| residual.incident_variables().contains(&variable_id))
        {
            return Err(CoreError::VariableInUse(variable_id));
        }
        self.variables
            .remove(variable_id)
            .ok_or(CoreError::UnknownVariable(variable_id))
    }

    #[must_use]
    pub fn variable(&self, variable_id: VariableId) -> Option<&VariableBlock> {
        self.variables.get(variable_id)
    }

    /// Replaces a variable's ambient value.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown ID, a kind mismatch, or non-finite data.
    pub fn set_variable_value(
        &mut self,
        variable_id: VariableId,
        value: VariableValue,
    ) -> Result<(), CoreError> {
        self.variables
            .get_mut(variable_id)
            .ok_or(CoreError::UnknownVariable(variable_id))?
            .set_value(value)
    }

    /// Applies a raw local tangent increment to one variable.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown ID, wrong dimension, or non-finite data.
    pub fn apply_local_increment(
        &mut self,
        variable_id: VariableId,
        delta: &[f64],
    ) -> Result<(), CoreError> {
        self.variables
            .get_mut(variable_id)
            .ok_or(CoreError::UnknownVariable(variable_id))?
            .apply_local_increment(delta)
    }

    pub fn add_source(&mut self, source: SourceConstraint) -> SourceConstraintId {
        self.sources.insert(source)
    }

    /// Removes an unreferenced source constraint.
    ///
    /// # Errors
    ///
    /// Returns an error if the ID is stale or a residual still references it.
    pub fn remove_source(
        &mut self,
        source_id: SourceConstraintId,
    ) -> Result<SourceConstraint, CoreError> {
        if self
            .residuals
            .iter()
            .any(|(_, residual)| residual.source() == source_id)
        {
            return Err(CoreError::SourceInUse(source_id));
        }
        self.sources
            .remove(source_id)
            .ok_or(CoreError::UnknownSource(source_id))
    }

    #[must_use]
    pub fn source(&self, source_id: SourceConstraintId) -> Option<&SourceConstraint> {
        self.sources.get(source_id)
    }

    /// Adds a validated additive tangent-coordinate box bound.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale variable, invalid coordinate, duplicate
    /// coordinate bound, or invalid interval. A finite initial guess outside
    /// the interval is projected to the nearest endpoint when solving starts.
    pub fn add_bound(&mut self, bound: CoordinateBound) -> Result<BoundId, CoreError> {
        bound.validate_for_problem(self)?;
        if self.bounds.iter().any(|(_, existing)| {
            existing.variable_id() == bound.variable_id()
                && existing.coordinate() == bound.coordinate()
        }) {
            return Err(CoreError::DuplicateBoundCoordinate {
                variable: bound.variable_id(),
                coordinate: bound.coordinate(),
            });
        }
        self.decomposition_cache = None;
        Ok(self.bounds.insert(bound))
    }

    #[must_use]
    pub fn bound(&self, bound_id: BoundId) -> Option<&CoordinateBound> {
        self.bounds.get(bound_id)
    }

    /// Returns bounds in deterministic insertion order.
    pub fn bounds(&self) -> impl Iterator<Item = (BoundId, &CoordinateBound)> {
        self.bounds.iter()
    }

    pub(crate) fn replace_source(
        &mut self,
        source_id: SourceConstraintId,
        source: SourceConstraint,
    ) -> Result<(), CoreError> {
        self.sources
            .replace(source_id, source)
            .ok_or(CoreError::UnknownSource(source_id))?;
        Ok(())
    }

    pub(crate) fn replace_residual_compatible(
        &mut self,
        residual_id: ResidualId,
        residual: ResidualBlock,
    ) -> Result<(), CoreError> {
        let current = self
            .residuals
            .get(residual_id)
            .ok_or(CoreError::UnknownResidual(residual_id))?;
        if let Some(field) = current.structurally_compatible_with(&residual) {
            return Err(CoreError::IncompatibleResidualReplacement {
                residual: residual_id,
                field,
            });
        }
        if let Some(crate::residual::ExactElimination::Fixed { variable_id, value }) =
            residual.exact_elimination()
        {
            let fixed = self
                .fixed_eliminations
                .iter_mut()
                .find(|fixed| fixed.residual_id == residual_id)
                .ok_or(CoreError::InvalidEliminationResidual {
                    residual: residual_id,
                    declaration: "fixed-variable",
                    message: "replacement has no matching fixed declaration",
                })?;
            fixed.variable_id = variable_id;
            fixed.value = value;
        }
        self.residuals
            .replace(residual_id, residual)
            .ok_or(CoreError::UnknownResidual(residual_id))?;
        Ok(())
    }

    pub(crate) fn replace_residual_audit_rows(
        &mut self,
        residual_id: ResidualId,
        audit_rows: Vec<crate::ResidualRowAudit>,
    ) -> Result<(), CoreError> {
        self.residuals
            .get_mut(residual_id)
            .ok_or(CoreError::UnknownResidual(residual_id))?
            .replace_audit_rows(audit_rows)
    }

    pub(crate) fn replace_bound_compatible(
        &mut self,
        bound_id: BoundId,
        bound: CoordinateBound,
    ) -> Result<(), CoreError> {
        let current = self
            .bounds
            .get(bound_id)
            .ok_or(CoreError::UnknownBound(bound_id))?;
        if current.variable_id() != bound.variable_id()
            || current.coordinate() != bound.coordinate()
        {
            return Err(CoreError::InvalidBoundCoordinate {
                variable: bound.variable_id(),
                coordinate: bound.coordinate(),
                dimension: self
                    .variable(bound.variable_id())
                    .map_or(0, |variable| variable.kind().tangent_dimension()),
            });
        }
        bound.validate_for_problem(self)?;
        self.bounds
            .replace(bound_id, bound)
            .ok_or(CoreError::UnknownBound(bound_id))?;
        Ok(())
    }

    /// Adds a residual after validating all declared IDs and incidence.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale source or variable ID, or duplicate
    /// incidence of the same variable.
    pub fn add_residual(&mut self, residual: ResidualBlock) -> Result<ResidualId, CoreError> {
        if self.sources.get(residual.source()).is_none() {
            return Err(CoreError::UnknownSource(residual.source()));
        }
        for (index, &variable_id) in residual.incident_variables().iter().enumerate() {
            if self.variables.get(variable_id).is_none() {
                return Err(CoreError::UnknownVariable(variable_id));
            }
            if residual.incident_variables()[..index].contains(&variable_id) {
                return Err(CoreError::DuplicateIncidentVariable(variable_id));
            }
        }
        Ok(self.residuals.insert(residual))
    }

    /// Removes a residual block.
    ///
    /// # Errors
    ///
    /// Returns an error if the ID is stale or unknown.
    pub fn remove_residual(&mut self, residual_id: ResidualId) -> Result<ResidualBlock, CoreError> {
        self.residuals
            .remove(residual_id)
            .ok_or(CoreError::UnknownResidual(residual_id))
    }

    #[must_use]
    pub fn residual(&self, residual_id: ResidualId) -> Option<&ResidualBlock> {
        self.residuals.get(residual_id)
    }

    /// Builds the current deterministic variable layout.
    ///
    /// # Errors
    ///
    /// Returns an error if variable data is invalid or dimensions overflow.
    pub fn packed_layout(&self) -> Result<PackedLayout, CoreError> {
        let mut blocks = Vec::new();
        let mut ambient_dimension = 0usize;
        let mut tangent_dimension = 0usize;
        for (variable_id, variable) in self.variables.iter() {
            variable.validate()?;
            let ambient_end = ambient_dimension
                .checked_add(variable.kind().ambient_dimension())
                .ok_or(CoreError::DimensionOverflow {
                    context: "packed ambient",
                })?;
            let tangent_end = tangent_dimension
                .checked_add(variable.kind().tangent_dimension())
                .ok_or(CoreError::DimensionOverflow {
                    context: "packed tangent",
                })?;
            blocks.push(BlockLayout {
                variable_id,
                kind: variable.kind(),
                ambient_range: ambient_dimension..ambient_end,
                tangent_range: tangent_dimension..tangent_end,
                step_scales: variable.step_scales().to_vec(),
            });
            ambient_dimension = ambient_end;
            tangent_dimension = tangent_end;
        }
        Ok(PackedLayout {
            blocks,
            ambient_dimension,
            tangent_dimension,
        })
    }

    /// Packs all ambient values in layout order.
    ///
    /// # Errors
    ///
    /// Returns an error if variable data is invalid or dimensions overflow.
    pub fn packed_state(&self) -> Result<PackedState, CoreError> {
        let layout = self.packed_layout()?;
        let mut ambient = Vec::with_capacity(layout.ambient_dimension());
        for (_, variable) in self.variables.iter() {
            ambient.extend_from_slice(variable.value().ambient_values());
        }
        Ok(PackedState {
            layout,
            ambient: DVector::from_vec(ambient),
        })
    }

    /// Flattens source and row audit metadata in executable residual row order.
    ///
    /// # Errors
    ///
    /// Returns an error if a residual references an unknown source.
    pub fn audit_rows(&self) -> Result<Vec<AuditRowDescriptor>, CoreError> {
        let mut descriptors = Vec::new();
        for (residual_id, residual) in self.residuals.iter() {
            let source = self
                .sources
                .get(residual.source())
                .ok_or(CoreError::UnknownSource(residual.source()))?;
            for (row_in_block, (audit, &scale)) in residual
                .audit_rows()
                .iter()
                .zip(residual.scales())
                .enumerate()
            {
                descriptors.push(AuditRowDescriptor {
                    residual_id,
                    source_id: residual.source(),
                    source_label: source.label().to_owned(),
                    category: residual.category(),
                    row_in_block,
                    template: audit.template.clone(),
                    bindings: audit.bindings.clone(),
                    unit: audit.unit.clone(),
                    scale,
                });
            }
        }
        Ok(descriptors)
    }

    /// Evaluates raw and normalized audit rows at the current state.
    ///
    /// # Errors
    ///
    /// Returns an error if any source, evaluator output, scale, or evaluated
    /// value is invalid. Rows are grouped in deterministic source-store order.
    pub fn audit_snapshot(&self) -> Result<AuditSnapshot, CoreError> {
        let state = self.variable_state();
        let mut snapshot = self.audit_source_shell();
        for (residual_id, residual) in self.residuals.iter() {
            let variables = Self::incident_values_from_state(residual, &state)?;
            let raw_values = evaluate_values(residual_id, residual, &variables)?;
            let normalized_values = normalize_residuals(residual, &raw_values)?;
            self.validate_residual_linearization(&state, residual_id)?;
            append_audit_rows(
                &mut snapshot,
                residual_id,
                residual,
                &variables,
                &raw_values,
                &normalized_values,
            )?;
        }
        Ok(snapshot)
    }

    /// Builds a best-effort audit, retaining fresh finite values on derivative failure.
    ///
    /// Rows are marked [`AuditEvaluationStatus::Evaluated`] only when both the
    /// fresh value evaluation and canonical Jacobian/fused linearization succeed.
    /// Failed rows retain a structured evaluator category when one is available.
    #[must_use]
    pub fn audit_snapshot_partial(&self) -> AuditSnapshot {
        self.audit_snapshot_partial_filtered(None)
    }

    pub(crate) fn audit_snapshot_partial_for_residuals(
        &self,
        residual_ids: &[ResidualId],
    ) -> AuditSnapshot {
        self.audit_snapshot_partial_filtered(Some(residual_ids))
    }

    fn audit_snapshot_partial_filtered(
        &self,
        residual_filter: Option<&[ResidualId]>,
    ) -> AuditSnapshot {
        let state = self.variable_state();
        let mut snapshot = self.audit_source_shell();
        for (residual_id, residual) in self.residuals.iter() {
            if residual_filter.is_some_and(|filter| !filter.contains(&residual_id)) {
                continue;
            }
            let variables = match Self::incident_values_from_state(residual, &state) {
                Ok(variables) => variables,
                Err(error) => {
                    append_failed_audit_rows(
                        &mut snapshot,
                        residual_id,
                        residual,
                        &[],
                        None,
                        &error.to_string(),
                        core_error_category(&error),
                    );
                    continue;
                }
            };
            let raw_values = match evaluate_values(residual_id, residual, &variables) {
                Ok(values) => values,
                Err(error) => {
                    append_failed_audit_rows(
                        &mut snapshot,
                        residual_id,
                        residual,
                        &variables,
                        None,
                        &error.to_string(),
                        core_error_category(&error),
                    );
                    continue;
                }
            };
            let normalized_values = match normalize_residuals(residual, &raw_values) {
                Ok(values) => values,
                Err(error) => {
                    append_failed_audit_rows(
                        &mut snapshot,
                        residual_id,
                        residual,
                        &variables,
                        None,
                        &error.to_string(),
                        core_error_category(&error),
                    );
                    continue;
                }
            };
            if let Err(error) = self.validate_residual_linearization(&state, residual_id) {
                append_failed_audit_rows(
                    &mut snapshot,
                    residual_id,
                    residual,
                    &variables,
                    Some((&raw_values, &normalized_values)),
                    &error.to_string(),
                    core_error_category(&error),
                );
                continue;
            }
            let _ = append_audit_rows(
                &mut snapshot,
                residual_id,
                residual,
                &variables,
                &raw_values,
                &normalized_values,
            );
        }
        snapshot
    }

    fn audit_source_shell(&self) -> AuditSnapshot {
        AuditSnapshot {
            sources: self
                .sources
                .iter()
                .filter(|(source_id, _)| {
                    self.residuals
                        .iter()
                        .any(|(_, residual)| residual.source() == *source_id)
                })
                .map(|(source_id, source)| AuditSourceSnapshot {
                    source_id,
                    source_label: source.label().to_owned(),
                    rows: Vec::new(),
                    annotations: AuditAnnotations::default(),
                    active_bounds: Vec::new(),
                })
                .collect(),
        }
    }

    /// Evaluates and validates all blocks before constructing dense matrices.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid IDs, dimensions, geometry, scales, or any
    /// non-finite raw or normalized value.
    pub fn assemble_dense(&self) -> Result<DenseAssembly, CoreError> {
        let state = self.variable_state();
        self.assemble_dense_for_state(&state)
    }

    pub(crate) fn assemble_dense_for_state(
        &self,
        state: &VariableState,
    ) -> Result<DenseAssembly, CoreError> {
        self.assemble_dense_for_state_filtered(state, None)
    }

    pub(crate) fn assemble_dense_for_state_filtered(
        &self,
        state: &VariableState,
        residual_filter: Option<&[ResidualId]>,
    ) -> Result<DenseAssembly, CoreError> {
        let variable_layout = self.packed_layout()?;
        let linearization = self.linearize_blocks_for_state(state, residual_filter)?;
        linearization
            .scalar_rows
            .checked_mul(variable_layout.tangent_dimension())
            .ok_or(CoreError::DimensionOverflow {
                context: "dense Jacobian",
            })?;
        let mut residual_values = Vec::with_capacity(linearization.scalar_rows);
        let mut jacobian = DMatrix::zeros(
            linearization.scalar_rows,
            variable_layout.tangent_dimension(),
        );
        let mut residual_layout = Vec::with_capacity(linearization.blocks.len());
        let mut row_start = 0usize;
        for block in linearization.blocks {
            let row_end = row_start + block.normalized_residuals.len();
            residual_values.extend_from_slice(&block.normalized_residuals);
            residual_layout.push(ResidualLayout {
                residual_id: block.residual_id,
                row_range: row_start..row_end,
            });
            for local in block.jacobian_blocks {
                let layout = variable_layout
                    .block(local.variable_id)
                    .ok_or(CoreError::UnknownVariable(local.variable_id))?;
                for local_row in 0..local.rows {
                    for local_column in 0..local.columns {
                        jacobian[(
                            row_start + local_row,
                            layout.tangent_range.start + local_column,
                        )] = local.normalized_values[local_row * local.columns + local_column];
                    }
                }
            }
            row_start = row_end;
        }

        Ok(DenseAssembly {
            variable_layout,
            residual_layout,
            residuals: DVector::from_vec(residual_values),
            jacobian,
        })
    }

    /// Compares normalized analytic blocks with central finite differences.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid step or any model, evaluator, dimension,
    /// geometry, or non-finite-data failure encountered during evaluation.
    pub fn check_jacobians(&self, normalized_step: f64) -> Result<JacobianCheckReport, CoreError> {
        if !normalized_step.is_finite()
            || normalized_step <= 0.0
            || !(2.0 * normalized_step).is_finite()
        {
            return Err(CoreError::InvalidFiniteDifferenceStep(normalized_step));
        }
        self.packed_layout()?;
        let mut reports = Vec::new();

        for (residual_id, residual) in self.residuals.iter() {
            let base_values = self.incident_values(residual)?;
            let state = self.variable_state();
            let linearization = self.linearize_blocks_for_state(&state, Some(&[residual_id]))?;
            let normalized_analytic = &linearization
                .blocks
                .first()
                .ok_or(CoreError::UnknownResidual(residual_id))?
                .jacobian_blocks;

            for (incident_index, (&variable_id, analytic_block)) in residual
                .incident_variables()
                .iter()
                .zip(normalized_analytic)
                .enumerate()
            {
                let variable = self
                    .variables
                    .get(variable_id)
                    .ok_or(CoreError::UnknownVariable(variable_id))?;
                let rows = residual.output_dimension();
                let columns = variable.kind().tangent_dimension();
                let mut max_absolute_error = 0.0_f64;
                let mut max_relative_error = 0.0_f64;
                let mut worst_row = 0usize;
                let mut worst_column = 0usize;

                for column in 0..columns {
                    let raw_step = normalized_step * variable.step_scales()[column];
                    if !raw_step.is_finite() {
                        return Err(CoreError::NonFiniteValue {
                            context: "finite-difference local step",
                            index: column,
                            value: raw_step,
                        });
                    }
                    let mut plus_values = base_values.clone();
                    let mut minus_values = base_values.clone();
                    let mut delta = vec![0.0; columns];
                    delta[column] = raw_step;
                    plus_values[incident_index].plus(&delta)?;
                    delta[column] = -raw_step;
                    minus_values[incident_index].plus(&delta)?;

                    let plus = normalize_residuals(
                        residual,
                        &evaluate_values(residual_id, residual, &plus_values)?,
                    )?;
                    let minus = normalize_residuals(
                        residual,
                        &evaluate_values(residual_id, residual, &minus_values)?,
                    )?;
                    for row in 0..rows {
                        let numeric = (plus[row] - minus[row]) / (2.0 * normalized_step);
                        if !numeric.is_finite() {
                            return Err(CoreError::NonFiniteValue {
                                context: "finite-difference Jacobian",
                                index: row * columns + column,
                                value: numeric,
                            });
                        }
                        let analytic_value =
                            analytic_block.normalized_values[row * columns + column];
                        let absolute_error = (analytic_value - numeric).abs();
                        let magnitude = analytic_value.abs().max(numeric.abs());
                        let relative_error = if magnitude > 1.0e-12 {
                            absolute_error / magnitude
                        } else {
                            absolute_error
                        };
                        max_absolute_error = max_absolute_error.max(absolute_error);
                        if relative_error > max_relative_error {
                            max_relative_error = relative_error;
                            worst_row = row;
                            worst_column = column;
                        }
                    }
                }
                reports.push(JacobianBlockReport {
                    residual_id,
                    variable_id,
                    rows,
                    columns,
                    max_absolute_error,
                    max_relative_error,
                    worst_row,
                    worst_column,
                });
            }
        }

        Ok(JacobianCheckReport {
            normalized_step,
            blocks: reports,
        })
    }

    fn incident_values(&self, residual: &ResidualBlock) -> Result<Vec<VariableValue>, CoreError> {
        residual
            .incident_variables()
            .iter()
            .map(|&variable_id| {
                self.variables
                    .get(variable_id)
                    .map(VariableBlock::value)
                    .ok_or(CoreError::UnknownVariable(variable_id))
            })
            .collect()
    }

    fn incident_values_from_state(
        residual: &ResidualBlock,
        state: &VariableState,
    ) -> Result<Vec<VariableValue>, CoreError> {
        residual
            .incident_variables()
            .iter()
            .map(|&variable_id| {
                state
                    .values
                    .iter()
                    .find_map(|&(id, value)| (id == variable_id).then_some(value))
                    .ok_or(CoreError::UnknownVariable(variable_id))
            })
            .collect()
    }

    pub(crate) fn variable_state(&self) -> VariableState {
        VariableState {
            values: self
                .variables
                .iter()
                .map(|(id, variable)| (id, variable.value()))
                .collect(),
        }
    }

    pub(crate) fn source_order(&self) -> Vec<SourceConstraintId> {
        self.sources.iter().map(|(id, _)| id).collect()
    }

    pub(crate) fn normalized_category_values_for_residuals(
        &self,
        state: &VariableState,
        category: ResidualCategory,
        residual_ids: &[ResidualId],
    ) -> Result<Vec<(ResidualId, usize, SourceConstraintId, f64)>, CoreError> {
        self.normalized_category_values_filtered(state, category, Some(residual_ids))
    }

    pub(crate) fn normalized_values_for_residuals(
        &self,
        state: &VariableState,
        residual_ids: &[ResidualId],
    ) -> Result<Vec<f64>, CoreError> {
        self.validate_variable_state(state)?;
        let mut values = Vec::new();
        for (residual_id, residual) in self.residuals.iter() {
            if !residual_ids.contains(&residual_id) {
                continue;
            }
            let variables = Self::incident_values_from_state(residual, state)?;
            let raw = evaluate_values(residual_id, residual, &variables)?;
            values.extend(normalize_residuals(residual, &raw)?);
        }
        Ok(values)
    }

    fn normalized_category_values_filtered(
        &self,
        state: &VariableState,
        category: ResidualCategory,
        residual_filter: Option<&[ResidualId]>,
    ) -> Result<Vec<(ResidualId, usize, SourceConstraintId, f64)>, CoreError> {
        self.validate_variable_state(state)?;
        let mut values = Vec::new();
        for (residual_id, residual) in self.residuals.iter() {
            if residual.category() != category
                || residual_filter.is_some_and(|filter| !filter.contains(&residual_id))
            {
                continue;
            }
            let variables = Self::incident_values_from_state(residual, state)?;
            let raw = evaluate_values(residual_id, residual, &variables)?;
            for (row, normalized) in normalize_residuals(residual, &raw)?.into_iter().enumerate() {
                values.push((residual_id, row, residual.source(), normalized));
            }
        }
        Ok(values)
    }

    pub(crate) fn replace_variable_state(
        &mut self,
        state: &VariableState,
    ) -> Result<(), CoreError> {
        self.validate_variable_state(state)?;
        for &(id, value) in &state.values {
            self.variables
                .get_mut(id)
                .ok_or(CoreError::UnknownVariable(id))?
                .set_value(value)?;
        }
        Ok(())
    }

    fn validate_variable_state(&self, state: &VariableState) -> Result<(), CoreError> {
        let expected = self.variables.iter().count();
        if state.values.len() != expected {
            return Err(CoreError::DimensionMismatch {
                context: "solver variable state",
                expected,
                actual: state.values.len(),
            });
        }
        for ((expected_id, variable), &(actual_id, value)) in
            self.variables.iter().zip(&state.values)
        {
            if actual_id != expected_id {
                return Err(CoreError::UnknownVariable(actual_id));
            }
            let expected_kind = variable.kind();
            let actual_kind = value.kind();
            if actual_kind != expected_kind {
                return Err(CoreError::VariableKindMismatch {
                    expected: expected_kind,
                    actual: actual_kind,
                });
            }
            value.validate_finite()?;
        }
        Ok(())
    }
}

fn append_audit_rows(
    snapshot: &mut AuditSnapshot,
    residual_id: ResidualId,
    residual: &ResidualBlock,
    variables: &[VariableValue],
    raw_values: &[f64],
    normalized_values: &[f64],
) -> Result<(), CoreError> {
    let incident_variables: Vec<_> = residual
        .incident_variables()
        .iter()
        .copied()
        .zip(variables.iter().copied())
        .map(|(variable_id, value)| AuditVariableSnapshot { variable_id, value })
        .collect();
    let source_snapshot = snapshot
        .sources
        .iter_mut()
        .find(|item| item.source_id == residual.source())
        .ok_or(CoreError::UnknownSource(residual.source()))?;
    for (row_in_block, audit) in residual.audit_rows().iter().enumerate() {
        source_snapshot.rows.push(AuditRowSnapshot {
            residual_id,
            category: residual.category(),
            row_in_block,
            template: audit.template.clone(),
            bindings: audit.bindings.clone(),
            incident_variables: incident_variables.clone(),
            unit: audit.unit.clone(),
            scale: residual.scales()[row_in_block],
            raw_residual: raw_values[row_in_block],
            normalized_residual: normalized_values[row_in_block],
            evaluation_status: AuditEvaluationStatus::Evaluated,
            evaluation_error_category: None,
            evaluation_error: None,
            annotations: AuditAnnotations::default(),
            active_bounds: Vec::new(),
        });
    }
    Ok(())
}

fn append_failed_audit_rows(
    snapshot: &mut AuditSnapshot,
    residual_id: ResidualId,
    residual: &ResidualBlock,
    variables: &[VariableValue],
    evaluated_values: Option<(&[f64], &[f64])>,
    error: &str,
    error_category: Option<EvaluationErrorCategory>,
) {
    let incident_variables: Vec<_> = residual
        .incident_variables()
        .iter()
        .copied()
        .zip(variables.iter().copied())
        .map(|(variable_id, value)| AuditVariableSnapshot { variable_id, value })
        .collect();
    let Some(source_snapshot) = snapshot
        .sources
        .iter_mut()
        .find(|item| item.source_id == residual.source())
    else {
        return;
    };
    for (row_in_block, audit) in residual.audit_rows().iter().enumerate() {
        let (raw_residual, normalized_residual) =
            evaluated_values.map_or((0.0, 0.0), |(raw_values, normalized_values)| {
                (raw_values[row_in_block], normalized_values[row_in_block])
            });
        source_snapshot.rows.push(AuditRowSnapshot {
            residual_id,
            category: residual.category(),
            row_in_block,
            template: audit.template.clone(),
            bindings: audit.bindings.clone(),
            incident_variables: incident_variables.clone(),
            unit: audit.unit.clone(),
            scale: residual.scales()[row_in_block],
            raw_residual,
            normalized_residual,
            evaluation_status: AuditEvaluationStatus::Failed,
            evaluation_error_category: error_category,
            evaluation_error: Some(error.to_owned()),
            annotations: AuditAnnotations::default(),
            active_bounds: Vec::new(),
        });
    }
}

fn core_error_category(error: &CoreError) -> Option<EvaluationErrorCategory> {
    match error {
        CoreError::CategorizedEvaluation { category, .. } => Some(*category),
        _ => None,
    }
}
