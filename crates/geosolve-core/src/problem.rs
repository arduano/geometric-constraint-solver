use std::ops::Range;

use nalgebra::{DMatrix, DVector};
use slotmap::{Key, SlotMap};

use crate::{
    AuditBinding, CoreError, EvaluationError, LocalJacobian, ResidualBlock, ResidualCategory,
    ResidualId, SourceConstraint, SourceConstraintId, VariableBlock, VariableId, VariableKind,
    VariableValue,
};

#[derive(Debug)]
struct StableStore<K: Key, V> {
    values: SlotMap<K, V>,
    insertion_order: Vec<K>,
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

    fn get(&self, key: K) -> Option<&V> {
        self.values.get(key)
    }

    fn get_mut(&mut self, key: K) -> Option<&mut V> {
        self.values.get_mut(key)
    }

    fn remove(&mut self, key: K) -> Option<V> {
        self.values.remove(key)
    }

    fn iter(&self) -> impl Iterator<Item = (K, &V)> {
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
#[derive(Debug)]
pub struct Problem {
    variables: StableStore<VariableId, VariableBlock>,
    residuals: StableStore<ResidualId, ResidualBlock>,
    sources: StableStore<SourceConstraintId, SourceConstraint>,
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
        let mut snapshot = AuditSnapshot::default();
        for (source_id, source) in self.sources.iter() {
            if self
                .residuals
                .iter()
                .any(|(_, residual)| residual.source() == source_id)
            {
                snapshot.sources.push(AuditSourceSnapshot {
                    source_id,
                    source_label: source.label().to_owned(),
                    rows: Vec::new(),
                });
            }
        }
        for (residual_id, residual) in self.residuals.iter() {
            let variables = Self::incident_values_from_state(residual, &state)?;
            let raw_values = evaluate_values(residual_id, residual, &variables)?;
            let normalized_values = normalize_residuals(residual, &raw_values)?;
            let incident_variables: Vec<_> = residual
                .incident_variables()
                .iter()
                .copied()
                .zip(variables.iter().copied())
                .map(|(variable_id, value)| AuditVariableSnapshot { variable_id, value })
                .collect();

            let source_index = snapshot
                .sources
                .iter()
                .position(|item| item.source_id == residual.source())
                .ok_or(CoreError::UnknownSource(residual.source()))?;
            let source_snapshot = &mut snapshot.sources[source_index];
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
                });
            }
        }
        Ok(snapshot)
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
        self.validate_variable_state(state)?;
        let variable_layout = self.packed_layout()?;
        let mut evaluated_blocks = Vec::new();
        let mut total_rows = 0usize;

        for (residual_id, residual) in self.residuals.iter() {
            let values = Self::incident_values_from_state(residual, state)?;
            let residual_values = evaluate_values(residual_id, residual, &values)?;
            let jacobians = evaluate_jacobians(residual_id, residual, &values, self)?;
            let normalized_values = normalize_residuals(residual, &residual_values)?;
            let normalized_jacobians = normalize_jacobians(residual, &jacobians, self)?;
            total_rows = total_rows.checked_add(residual.output_dimension()).ok_or(
                CoreError::DimensionOverflow {
                    context: "packed residual",
                },
            )?;
            evaluated_blocks.push(EvaluatedBlock {
                residual_id,
                incident_variables: residual.incident_variables().to_vec(),
                normalized_values,
                normalized_jacobians,
            });
        }

        let mut residual_values = Vec::with_capacity(total_rows);
        let mut jacobian = DMatrix::zeros(total_rows, variable_layout.tangent_dimension());
        let mut residual_layout = Vec::with_capacity(evaluated_blocks.len());
        let mut row_start = 0usize;
        for block in evaluated_blocks {
            let row_end = row_start + block.normalized_values.len();
            residual_values.extend_from_slice(&block.normalized_values);
            residual_layout.push(ResidualLayout {
                residual_id: block.residual_id,
                row_range: row_start..row_end,
            });
            for (&variable_id, local) in block
                .incident_variables
                .iter()
                .zip(&block.normalized_jacobians)
            {
                let layout = variable_layout
                    .block(variable_id)
                    .ok_or(CoreError::UnknownVariable(variable_id))?;
                for local_row in 0..local.rows() {
                    for local_column in 0..local.columns() {
                        jacobian[(
                            row_start + local_row,
                            layout.tangent_range.start + local_column,
                        )] = local.values()[local_row * local.columns() + local_column];
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
            let base_residuals = evaluate_values(residual_id, residual, &base_values)?;
            let _normalized_base = normalize_residuals(residual, &base_residuals)?;
            let analytic = evaluate_jacobians(residual_id, residual, &base_values, self)?;
            let normalized_analytic = normalize_jacobians(residual, &analytic, self)?;

            for (incident_index, (&variable_id, analytic_block)) in residual
                .incident_variables()
                .iter()
                .zip(&normalized_analytic)
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
                        let analytic_value = analytic_block.values()[row * columns + column];
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

    pub(crate) fn normalized_category_values(
        &self,
        state: &VariableState,
        category: ResidualCategory,
    ) -> Result<Vec<(ResidualId, usize, SourceConstraintId, f64)>, CoreError> {
        self.validate_variable_state(state)?;
        let mut values = Vec::new();
        for (residual_id, residual) in self.residuals.iter() {
            if residual.category() != category {
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

#[derive(Debug)]
struct EvaluatedBlock {
    residual_id: ResidualId,
    incident_variables: Vec<VariableId>,
    normalized_values: Vec<f64>,
    normalized_jacobians: Vec<LocalJacobian>,
}

fn evaluate_values(
    residual_id: ResidualId,
    residual: &ResidualBlock,
    variables: &[VariableValue],
) -> Result<Vec<f64>, CoreError> {
    let values = residual
        .evaluator()
        .evaluate(variables)
        .map_err(|error| evaluator_error(residual_id, error))?;
    if values.len() != residual.output_dimension() {
        return Err(CoreError::DimensionMismatch {
            context: "evaluator residual output",
            expected: residual.output_dimension(),
            actual: values.len(),
        });
    }
    validate_finite(&values, "evaluator residual output")?;
    Ok(values)
}

fn evaluate_jacobians(
    residual_id: ResidualId,
    residual: &ResidualBlock,
    variables: &[VariableValue],
    problem: &Problem,
) -> Result<Vec<LocalJacobian>, CoreError> {
    let jacobians = residual
        .evaluator()
        .jacobian(variables)
        .map_err(|error| evaluator_error(residual_id, error))?;
    if jacobians.len() != residual.incident_variables().len() {
        return Err(CoreError::DimensionMismatch {
            context: "evaluator Jacobian block count",
            expected: residual.incident_variables().len(),
            actual: jacobians.len(),
        });
    }
    for (&variable_id, jacobian) in residual.incident_variables().iter().zip(&jacobians) {
        let variable = problem
            .variables
            .get(variable_id)
            .ok_or(CoreError::UnknownVariable(variable_id))?;
        if jacobian.rows() != residual.output_dimension() {
            return Err(CoreError::DimensionMismatch {
                context: "local Jacobian rows",
                expected: residual.output_dimension(),
                actual: jacobian.rows(),
            });
        }
        let columns = variable.kind().tangent_dimension();
        if jacobian.columns() != columns {
            return Err(CoreError::DimensionMismatch {
                context: "local Jacobian columns",
                expected: columns,
                actual: jacobian.columns(),
            });
        }
        let expected_values =
            jacobian
                .rows()
                .checked_mul(columns)
                .ok_or(CoreError::DimensionOverflow {
                    context: "local Jacobian",
                })?;
        if jacobian.values().len() != expected_values {
            return Err(CoreError::DimensionMismatch {
                context: "local Jacobian values",
                expected: expected_values,
                actual: jacobian.values().len(),
            });
        }
        validate_finite(jacobian.values(), "evaluator Jacobian")?;
    }
    Ok(jacobians)
}

fn normalize_residuals(residual: &ResidualBlock, values: &[f64]) -> Result<Vec<f64>, CoreError> {
    values
        .iter()
        .zip(residual.scales())
        .enumerate()
        .map(|(index, (&value, &scale))| {
            let normalized = value / scale;
            if normalized.is_finite() {
                Ok(normalized)
            } else {
                Err(CoreError::NonFiniteValue {
                    context: "normalized residual",
                    index,
                    value: normalized,
                })
            }
        })
        .collect()
}

fn normalize_jacobians(
    residual: &ResidualBlock,
    jacobians: &[LocalJacobian],
    problem: &Problem,
) -> Result<Vec<LocalJacobian>, CoreError> {
    residual
        .incident_variables()
        .iter()
        .zip(jacobians)
        .map(|(&variable_id, jacobian)| {
            let variable = problem
                .variables
                .get(variable_id)
                .ok_or(CoreError::UnknownVariable(variable_id))?;
            let mut values = Vec::with_capacity(jacobian.values().len());
            for row in 0..jacobian.rows() {
                for column in 0..jacobian.columns() {
                    let raw = jacobian.values()[row * jacobian.columns() + column];
                    let normalized = raw * variable.step_scales()[column] / residual.scales()[row];
                    if !normalized.is_finite() {
                        return Err(CoreError::NonFiniteValue {
                            context: "normalized Jacobian",
                            index: row * jacobian.columns() + column,
                            value: normalized,
                        });
                    }
                    values.push(normalized);
                }
            }
            Ok(LocalJacobian::new(
                jacobian.rows(),
                jacobian.columns(),
                values,
            ))
        })
        .collect()
}

fn validate_finite(values: &[f64], context: &'static str) -> Result<(), CoreError> {
    for (index, &value) in values.iter().enumerate() {
        if !value.is_finite() {
            return Err(CoreError::NonFiniteValue {
                context,
                index,
                value,
            });
        }
    }
    Ok(())
}

fn evaluator_error(residual: ResidualId, error: EvaluationError) -> CoreError {
    match error {
        EvaluationError::InvalidGeometry(message) => {
            CoreError::InvalidGeometry { residual, message }
        }
    }
}
