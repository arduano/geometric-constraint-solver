use crate::linearization::{ComponentTangentLayout, component_tangent_layout};
use crate::problem::VariableState;
use crate::residual::ExactElimination;
use crate::{
    CoreError, Problem, ResidualCategory, ResidualId, ResidualRowRef, SourceConstraintId,
    VariableId, VariableKind, VariableValue,
};
use std::collections::VecDeque;

/// One declared variable-to-residual edge in deterministic residual/incidence order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IncidenceEdge {
    pub variable_id: VariableId,
    pub residual_id: ResidualId,
}

/// One connected component of the original variable/residual bipartite graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IncidenceComponent {
    pub index: usize,
    pub variable_ids: Vec<VariableId>,
    pub residual_ids: Vec<ResidualId>,
}

/// Original declared incidence, retained independently from reduced solve components.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct IncidenceAnalysis {
    pub variable_ids: Vec<VariableId>,
    pub residual_ids: Vec<ResidualId>,
    pub edges: Vec<IncidenceEdge>,
    pub components: Vec<IncidenceComponent>,
}

/// Structural under/well/over classification of the declared reduced hard
/// block envelope. Numerical zeros do not remove declared incidence slots.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum StructuralClassification {
    Under,
    #[default]
    Well,
    Over,
    Mixed,
}

/// One normalized tangent coordinate, identified through its reduced alias root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TangentCoordinateRef {
    pub root: VariableId,
    pub coordinate_in_block: usize,
}

/// Rows and tangent coordinates in one Dulmage-Mendelsohn part.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DulmageMendelsohnPartition {
    pub rows: Vec<ResidualRowRef>,
    pub tangent_coordinates: Vec<TangentCoordinateRef>,
}

/// Canonically ordered Dulmage-Mendelsohn parts of a reduced hard system.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DulmageMendelsohnPartitions {
    pub under: DulmageMendelsohnPartition,
    pub well: DulmageMendelsohnPartition,
    pub over: DulmageMendelsohnPartition,
}

/// Structural counts and signature for one reduced solve component.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComponentStructuralSummary {
    pub component_index: usize,
    /// Existing topology/cache signature, including scales and declared order.
    pub pattern_signature: u64,
    /// Scale- and value-independent reduced block-envelope sparsity signature.
    pub sparsity_signature: u64,
    pub variable_ids: Vec<VariableId>,
    pub residual_ids: Vec<ResidualId>,
    pub variable_blocks: usize,
    pub tangent_dimensions: usize,
    pub residual_blocks: usize,
    pub scalar_rows: usize,
    pub fixed_eliminated_coordinates: usize,
    pub aliased_eliminated_coordinates: usize,
    pub eliminated_rows: usize,
    pub active_tangent_dimensions: usize,
    pub active_rows: usize,
    pub active_hard_rows: usize,
    /// Canonical reduced block-envelope entries, including explicit zero slots.
    pub structural_nnz: usize,
    /// Maximum matching rank of the declared block envelope, not numerical rank.
    pub structural_rank: usize,
    pub structural_left_nullity: usize,
    pub structural_right_nullity: usize,
    pub structural_classification: StructuralClassification,
    pub dm_partitions: DulmageMendelsohnPartitions,
}

/// Whole-problem structural facts, separate from numerical Jacobian rank.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StructuralSummary {
    /// Scale- and value-independent signature of all declared block envelopes.
    pub sparsity_signature: u64,
    pub variable_blocks: usize,
    pub tangent_dimensions: usize,
    pub residual_blocks: usize,
    pub scalar_rows: usize,
    pub components: usize,
    pub fixed_eliminated_coordinates: usize,
    pub aliased_eliminated_coordinates: usize,
    pub eliminated_rows: usize,
    pub active_tangent_dimensions: usize,
    pub active_rows: usize,
    pub active_hard_rows: usize,
    /// Sum of canonical reduced block-envelope entries, including explicit zeros.
    pub structural_nnz: usize,
    /// Sum of component maximum-matching ranks for the declared block envelopes.
    pub structural_rank: usize,
    pub structural_left_nullity: usize,
    pub structural_right_nullity: usize,
    pub structural_classification: StructuralClassification,
    pub dm_partitions: DulmageMendelsohnPartitions,
    pub component_summaries: Vec<ComponentStructuralSummary>,
}

#[derive(Clone, Debug)]
pub(crate) struct FixedElimination {
    pub(crate) variable_id: VariableId,
    pub(crate) value: VariableValue,
    pub(crate) residual_id: ResidualId,
}

#[derive(Clone, Debug)]
pub(crate) struct AliasElimination {
    pub(crate) alias: VariableId,
    pub(crate) representative: VariableId,
    pub(crate) residual_id: ResidualId,
}

#[derive(Clone, Debug)]
pub(crate) struct CachedComponent {
    pub(crate) pattern_signature: u64,
    pub(crate) variable_ids: Vec<VariableId>,
    pub(crate) residual_ids: Vec<ResidualId>,
    pub(crate) values: Vec<VariableValue>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct DecompositionCache {
    pub(crate) components: Vec<CachedComponent>,
    pub(crate) report: Option<Box<crate::SolveReport>>,
}

#[derive(Clone, Debug)]
pub(crate) struct ActiveGroup {
    pub(crate) root: VariableId,
    pub(crate) members: Vec<VariableId>,
    pub(crate) component_index: usize,
    pub(crate) kind: VariableKind,
    pub(crate) step_scales: Vec<f64>,
}

#[derive(Clone, Debug)]
pub(crate) struct SolveComponent {
    pub(crate) index: usize,
    pub(crate) active_group_indices: Vec<usize>,
    pub(crate) variable_ids: Vec<VariableId>,
    pub(crate) residual_ids: Vec<ResidualId>,
    pub(crate) active_residual_ids: Vec<ResidualId>,
    pub(crate) referenced_variables: Vec<VariableId>,
}

#[derive(Clone, Debug)]
pub(crate) struct EliminationPlan {
    pub(crate) roots: Vec<(VariableId, VariableId)>,
    pub(crate) active_groups: Vec<ActiveGroup>,
    pub(crate) eliminated_residuals: Vec<ResidualId>,
    pub(crate) components: Vec<SolveComponent>,
    pub(crate) component_layouts: Vec<ComponentTangentLayout>,
    pub(crate) structural: StructuralSummary,
    suppressed_sources: Vec<SourceConstraintId>,
}

impl Problem {
    /// Returns the original declared incidence graph, including isolated nodes.
    #[must_use]
    pub fn analyze_incidence(&self) -> IncidenceAnalysis {
        let variable_ids: Vec<_> = self.variables.iter().map(|(id, _)| id).collect();
        let residual_ids: Vec<_> = self.residuals.iter().map(|(id, _)| id).collect();
        let mut edges = Vec::new();
        let node_count = variable_ids.len() + residual_ids.len();
        let mut parents: Vec<_> = (0..node_count).collect();

        for (residual_index, &residual_id) in residual_ids.iter().enumerate() {
            let Some(residual) = self.residuals.get(residual_id) else {
                continue;
            };
            for &variable_id in residual.incident_variables() {
                edges.push(IncidenceEdge {
                    variable_id,
                    residual_id,
                });
                if let Some(variable_index) = variable_ids.iter().position(|&id| id == variable_id)
                {
                    union(
                        &mut parents,
                        variable_index,
                        variable_ids.len() + residual_index,
                    );
                }
            }
        }

        let mut roots = Vec::new();
        for node in 0..node_count {
            let root = find_root(&mut parents, node);
            if !roots.contains(&root) {
                roots.push(root);
            }
        }
        roots.sort_unstable();
        let components = roots
            .into_iter()
            .enumerate()
            .map(|(index, root)| IncidenceComponent {
                index,
                variable_ids: variable_ids
                    .iter()
                    .enumerate()
                    .filter_map(|(node, &id)| (find_root(&mut parents, node) == root).then_some(id))
                    .collect(),
                residual_ids: residual_ids
                    .iter()
                    .enumerate()
                    .filter_map(|(residual_index, &id)| {
                        (find_root(&mut parents, variable_ids.len() + residual_index) == root)
                            .then_some(id)
                    })
                    .collect(),
            })
            .collect();

        IncidenceAnalysis {
            variable_ids,
            residual_ids,
            edges,
            components,
        }
    }

    /// Declares that one trusted exact hard residual fixes an entire variable block.
    ///
    /// # Errors
    ///
    /// Returns an error for stale IDs, invalid fixed data, a marker/value
    /// mismatch, or conflicting elimination declarations.
    pub fn declare_fixed_variable(
        &mut self,
        variable_id: VariableId,
        value: VariableValue,
        residual_id: ResidualId,
    ) -> Result<(), CoreError> {
        let value = value.canonicalized()?;
        self.fixed_eliminations.push(FixedElimination {
            variable_id,
            value,
            residual_id,
        });
        let plan = match EliminationPlan::new(self) {
            Ok(plan) => plan,
            Err(error) => {
                self.fixed_eliminations.pop();
                return Err(error);
            }
        };
        let mut state = self.variable_state();
        plan.synchronize_state(self, &mut state)?;
        self.replace_variable_state(&state)
    }

    /// Declares that one trusted exact hard residual enforces an alias relationship.
    ///
    /// # Errors
    ///
    /// Returns an error for stale IDs, incompatible kinds/scales, a marker/ID
    /// mismatch, conflicting representatives, or an alias cycle.
    pub fn declare_exact_alias(
        &mut self,
        alias: VariableId,
        representative: VariableId,
        residual_id: ResidualId,
    ) -> Result<(), CoreError> {
        self.alias_eliminations.push(AliasElimination {
            alias,
            representative,
            residual_id,
        });
        let plan = match EliminationPlan::new(self) {
            Ok(plan) => plan,
            Err(error) => {
                self.alias_eliminations.pop();
                return Err(error);
            }
        };
        let mut state = self.variable_state();
        plan.synchronize_state(self, &mut state)?;
        self.replace_variable_state(&state)
    }

    /// Returns validated counts for the reduced hard solve graph.
    ///
    /// # Errors
    ///
    /// Returns an error if an elimination declaration is stale or invalid.
    pub fn structural_summary(&self) -> Result<StructuralSummary, CoreError> {
        Ok(EliminationPlan::new(self)?.structural)
    }
}

impl EliminationPlan {
    pub(crate) fn new(problem: &Problem) -> Result<Self, CoreError> {
        Self::new_suppressed(problem, &[])
    }

    pub(crate) fn new_suppressed(
        problem: &Problem,
        suppressed_sources: &[SourceConstraintId],
    ) -> Result<Self, CoreError> {
        validate_declarations(problem)?;
        let incidence = problem.analyze_incidence();
        let mut roots = Vec::with_capacity(incidence.variable_ids.len());
        for &variable_id in &incidence.variable_ids {
            roots.push((
                variable_id,
                alias_root(problem, variable_id, suppressed_sources)?,
            ));
        }
        let fixed_roots: Vec<_> = problem
            .fixed_eliminations
            .iter()
            .filter(|fixed| {
                !declaration_is_suppressed(problem, fixed.residual_id, suppressed_sources)
            })
            .map(|fixed| fixed.variable_id)
            .collect();
        let mut active_groups = build_active_groups(problem, &incidence, &roots, &fixed_roots)?;
        let eliminated_residuals = active_eliminated_residuals(problem, suppressed_sources);
        let mut components = build_reduced_components(
            problem,
            &incidence,
            &roots,
            &mut active_groups,
            &fixed_roots,
            &eliminated_residuals,
            suppressed_sources,
        )?;
        sort_components(&incidence, &mut active_groups, &mut components);
        let structural = structural_summary(
            problem,
            &incidence,
            &active_groups,
            &components,
            &eliminated_residuals,
        )?;
        let mut plan = Self {
            roots,
            active_groups,
            eliminated_residuals,
            components,
            component_layouts: Vec::new(),
            structural,
            suppressed_sources: suppressed_sources.to_vec(),
        };
        plan.component_layouts = plan
            .components
            .iter()
            .map(|component| component_tangent_layout(&plan, component.index))
            .collect();
        Ok(plan)
    }

    pub(crate) fn root(&self, variable_id: VariableId) -> Option<VariableId> {
        root_for(&self.roots, variable_id)
    }

    pub(crate) fn component_for_variable(&self, variable_id: VariableId) -> Option<usize> {
        self.components
            .iter()
            .find(|component| component.variable_ids.contains(&variable_id))
            .map(|component| component.index)
    }

    pub(crate) fn is_eliminated(&self, residual_id: ResidualId) -> bool {
        self.eliminated_residuals.contains(&residual_id)
    }

    pub(crate) fn source_is_suppressed(&self, source: SourceConstraintId) -> bool {
        self.suppressed_sources.contains(&source)
    }

    pub(crate) fn synchronize_state(
        &self,
        problem: &Problem,
        state: &mut VariableState,
    ) -> Result<(), CoreError> {
        for fixed in &problem.fixed_eliminations {
            if declaration_is_suppressed(problem, fixed.residual_id, &self.suppressed_sources) {
                continue;
            }
            set_state_value(state, fixed.variable_id, fixed.value)?;
        }
        for alias in &problem.alias_eliminations {
            if declaration_is_suppressed(problem, alias.residual_id, &self.suppressed_sources) {
                continue;
            }
            let root = self
                .root(alias.alias)
                .ok_or(CoreError::UnknownVariable(alias.alias))?;
            let value = state_value(state, root).ok_or(CoreError::UnknownVariable(root))?;
            set_state_value(state, alias.alias, value)?;
        }
        Ok(())
    }
}

fn build_active_groups(
    problem: &Problem,
    incidence: &IncidenceAnalysis,
    roots: &[(VariableId, VariableId)],
    fixed_roots: &[VariableId],
) -> Result<Vec<ActiveGroup>, CoreError> {
    let mut groups: Vec<ActiveGroup> = Vec::new();
    for &variable_id in &incidence.variable_ids {
        let root = root_for(roots, variable_id).ok_or(CoreError::UnknownVariable(variable_id))?;
        if fixed_roots.contains(&root) {
            continue;
        }
        if let Some(group) = groups.iter_mut().find(|group| group.root == root) {
            group.members.push(variable_id);
            continue;
        }
        let variable = problem
            .variables
            .get(root)
            .ok_or(CoreError::UnknownVariable(root))?;
        groups.push(ActiveGroup {
            root,
            members: vec![variable_id],
            component_index: 0,
            kind: variable.kind(),
            step_scales: variable.step_scales().to_vec(),
        });
    }
    Ok(groups)
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn build_reduced_components(
    problem: &Problem,
    incidence: &IncidenceAnalysis,
    roots: &[(VariableId, VariableId)],
    active_groups: &mut [ActiveGroup],
    fixed_roots: &[VariableId],
    eliminated_residuals: &[ResidualId],
    suppressed_sources: &[SourceConstraintId],
) -> Result<Vec<SolveComponent>, CoreError> {
    let active_residuals: Vec<_> = problem
        .residuals
        .iter()
        .filter(|(residual_id, residual)| {
            residual.category() == ResidualCategory::Hard
                && !eliminated_residuals.contains(residual_id)
                && !suppressed_sources.contains(&residual.source())
        })
        .map(|(id, _)| id)
        .collect();
    let node_count = active_groups.len() + active_residuals.len();
    let mut parents: Vec<_> = (0..node_count).collect();
    for (residual_index, &residual_id) in active_residuals.iter().enumerate() {
        let residual = problem
            .residuals
            .get(residual_id)
            .ok_or(CoreError::UnknownResidual(residual_id))?;
        for &variable_id in residual.incident_variables() {
            let root =
                root_for(roots, variable_id).ok_or(CoreError::UnknownVariable(variable_id))?;
            if let Some(group_index) = active_groups.iter().position(|group| group.root == root) {
                union(
                    &mut parents,
                    group_index,
                    active_groups.len() + residual_index,
                );
            }
        }
    }
    let mut component_roots = Vec::new();
    for node in 0..node_count {
        let root = find_root(&mut parents, node);
        if !component_roots.contains(&root) {
            component_roots.push(root);
        }
    }
    component_roots.sort_unstable();
    let mut components = Vec::new();
    for root in component_roots {
        let group_indices: Vec<_> = (0..active_groups.len())
            .filter(|&index| find_root(&mut parents, index) == root)
            .collect();
        let residual_ids: Vec<_> = active_residuals
            .iter()
            .enumerate()
            .filter_map(|(index, &residual_id)| {
                (find_root(&mut parents, active_groups.len() + index) == root)
                    .then_some(residual_id)
            })
            .collect();
        let variable_ids = incidence
            .variable_ids
            .iter()
            .copied()
            .filter(|variable_id| {
                group_indices
                    .iter()
                    .any(|&index| active_groups[index].members.contains(variable_id))
            })
            .collect();
        let referenced_variables = referenced_variables(problem, incidence, &residual_ids);
        components.push(SolveComponent {
            index: components.len(),
            active_group_indices: group_indices,
            variable_ids,
            residual_ids: residual_ids.clone(),
            active_residual_ids: residual_ids,
            referenced_variables,
        });
    }

    for &fixed_root in fixed_roots {
        let variable_ids: Vec<_> = incidence
            .variable_ids
            .iter()
            .copied()
            .filter(|&variable| root_for(roots, variable) == Some(fixed_root))
            .collect();
        let residual_ids: Vec<_> = eliminated_residuals
            .iter()
            .copied()
            .filter(|&residual_id| {
                problem.residuals.get(residual_id).is_some_and(|residual| {
                    residual
                        .incident_variables()
                        .iter()
                        .any(|&variable| root_for(roots, variable) == Some(fixed_root))
                })
            })
            .collect();
        components.push(SolveComponent {
            index: components.len(),
            active_group_indices: Vec::new(),
            variable_ids: variable_ids.clone(),
            residual_ids,
            active_residual_ids: Vec::new(),
            referenced_variables: variable_ids,
        });
    }

    for alias in &problem.alias_eliminations {
        if declaration_is_suppressed(problem, alias.residual_id, suppressed_sources) {
            continue;
        }
        let root = root_for(roots, alias.alias).ok_or(CoreError::UnknownVariable(alias.alias))?;
        if fixed_roots.contains(&root) {
            continue;
        }
        if let Some(component) = components.iter_mut().find(|component| {
            component
                .active_group_indices
                .iter()
                .any(|&group_index| active_groups[group_index].root == root)
        }) {
            push_unique(&mut component.residual_ids, alias.residual_id);
            push_unique(&mut component.referenced_variables, alias.alias);
            push_unique(&mut component.referenced_variables, alias.representative);
        }
    }
    Ok(components)
}

fn sort_components(
    incidence: &IncidenceAnalysis,
    active_groups: &mut [ActiveGroup],
    components: &mut [SolveComponent],
) {
    components.sort_by_key(|component| component_sort_key(incidence, component));
    for (index, component) in components.iter_mut().enumerate() {
        component.index = index;
        component.variable_ids.sort_by_key(|id| {
            incidence
                .variable_ids
                .iter()
                .position(|candidate| candidate == id)
                .unwrap_or(usize::MAX)
        });
        component.residual_ids.sort_by_key(|id| {
            incidence
                .residual_ids
                .iter()
                .position(|candidate| candidate == id)
                .unwrap_or(usize::MAX)
        });
        component.active_residual_ids.sort_by_key(|id| {
            incidence
                .residual_ids
                .iter()
                .position(|candidate| candidate == id)
                .unwrap_or(usize::MAX)
        });
        component.referenced_variables.sort_by_key(|id| {
            incidence
                .variable_ids
                .iter()
                .position(|candidate| candidate == id)
                .unwrap_or(usize::MAX)
        });
        for &group_index in &component.active_group_indices {
            active_groups[group_index].component_index = index;
        }
    }
}

fn component_sort_key(incidence: &IncidenceAnalysis, component: &SolveComponent) -> usize {
    let variable_key = component
        .variable_ids
        .iter()
        .filter_map(|id| {
            incidence
                .variable_ids
                .iter()
                .position(|candidate| candidate == id)
        })
        .min();
    let residual_key = component
        .residual_ids
        .iter()
        .filter_map(|id| {
            incidence
                .residual_ids
                .iter()
                .position(|candidate| candidate == id)
        })
        .min()
        .map(|index| incidence.variable_ids.len() + index);
    variable_key
        .into_iter()
        .chain(residual_key)
        .min()
        .unwrap_or(usize::MAX)
}

fn referenced_variables(
    problem: &Problem,
    incidence: &IncidenceAnalysis,
    residual_ids: &[ResidualId],
) -> Vec<VariableId> {
    let mut referenced = Vec::new();
    for &variable_id in &incidence.variable_ids {
        if residual_ids.iter().any(|&residual_id| {
            problem
                .residuals
                .get(residual_id)
                .is_some_and(|residual| residual.incident_variables().contains(&variable_id))
        }) {
            referenced.push(variable_id);
        }
    }
    referenced
}

#[allow(clippy::too_many_lines)]
fn validate_declarations(problem: &Problem) -> Result<(), CoreError> {
    let mut used_residuals = Vec::new();
    let mut fixed_variables = Vec::new();
    for fixed in &problem.fixed_eliminations {
        let variable = problem
            .variables
            .get(fixed.variable_id)
            .ok_or(CoreError::UnknownVariable(fixed.variable_id))?;
        if fixed.value.kind() != variable.kind() {
            return Err(CoreError::VariableKindMismatch {
                expected: variable.kind(),
                actual: fixed.value.kind(),
            });
        }
        fixed.value.validate_finite()?;
        if fixed_variables.contains(&fixed.variable_id) {
            return Err(CoreError::ConflictingElimination {
                variable: fixed.variable_id,
                message: "variable is fixed more than once",
            });
        }
        if problem
            .alias_eliminations
            .iter()
            .any(|alias| alias.alias == fixed.variable_id)
        {
            return Err(CoreError::ConflictingElimination {
                variable: fixed.variable_id,
                message: "a fixed variable cannot also be an alias",
            });
        }
        let residual = problem
            .residuals
            .get(fixed.residual_id)
            .ok_or(CoreError::UnknownResidual(fixed.residual_id))?;
        if residual.exact_elimination()
            != Some(ExactElimination::Fixed {
                variable_id: fixed.variable_id,
                value: fixed.value,
            })
        {
            return Err(CoreError::InvalidEliminationResidual {
                residual: fixed.residual_id,
                declaration: "fixed-variable",
                message: "trusted fixed marker does not match the declared variable and value",
            });
        }
        reject_reused_elimination(&used_residuals, fixed.residual_id, "fixed-variable")?;
        fixed_variables.push(fixed.variable_id);
        used_residuals.push(fixed.residual_id);
    }

    let mut aliases = Vec::new();
    for alias in &problem.alias_eliminations {
        let alias_variable = problem
            .variables
            .get(alias.alias)
            .ok_or(CoreError::UnknownVariable(alias.alias))?;
        let representative = problem
            .variables
            .get(alias.representative)
            .ok_or(CoreError::UnknownVariable(alias.representative))?;
        if alias.alias == alias.representative {
            return Err(CoreError::AliasCycle {
                variable: alias.alias,
            });
        }
        if alias_variable.kind() != representative.kind() {
            return Err(CoreError::VariableKindMismatch {
                expected: representative.kind(),
                actual: alias_variable.kind(),
            });
        }
        if alias_variable.step_scales() != representative.step_scales() {
            return Err(CoreError::AliasScaleMismatch {
                alias: alias.alias,
                representative: alias.representative,
            });
        }
        if aliases.contains(&alias.alias) {
            return Err(CoreError::ConflictingElimination {
                variable: alias.alias,
                message: "variable has more than one alias representative",
            });
        }
        let residual = problem
            .residuals
            .get(alias.residual_id)
            .ok_or(CoreError::UnknownResidual(alias.residual_id))?;
        if residual.exact_elimination()
            != Some(ExactElimination::Alias {
                alias: alias.alias,
                representative: alias.representative,
                kind: alias_variable.kind(),
            })
        {
            return Err(CoreError::InvalidEliminationResidual {
                residual: alias.residual_id,
                declaration: "exact-alias",
                message: "trusted alias marker does not match the declared IDs and kind",
            });
        }
        reject_reused_elimination(&used_residuals, alias.residual_id, "exact-alias")?;
        aliases.push(alias.alias);
        used_residuals.push(alias.residual_id);
    }
    for &variable_id in &aliases {
        alias_root(problem, variable_id, &[])?;
    }
    Ok(())
}

fn reject_reused_elimination(
    used_residuals: &[ResidualId],
    residual_id: ResidualId,
    declaration: &'static str,
) -> Result<(), CoreError> {
    if used_residuals.contains(&residual_id) {
        Err(CoreError::InvalidEliminationResidual {
            residual: residual_id,
            declaration,
            message: "residual block is already used by another elimination",
        })
    } else {
        Ok(())
    }
}

fn active_eliminated_residuals(
    problem: &Problem,
    suppressed_sources: &[SourceConstraintId],
) -> Vec<ResidualId> {
    let mut residuals = Vec::new();
    for residual_id in problem
        .fixed_eliminations
        .iter()
        .map(|fixed| fixed.residual_id)
        .chain(
            problem
                .alias_eliminations
                .iter()
                .map(|alias| alias.residual_id),
        )
    {
        if !declaration_is_suppressed(problem, residual_id, suppressed_sources) {
            residuals.push(residual_id);
        }
    }
    residuals
}

fn declaration_is_suppressed(
    problem: &Problem,
    residual_id: ResidualId,
    suppressed_sources: &[SourceConstraintId],
) -> bool {
    problem
        .residuals
        .get(residual_id)
        .is_some_and(|residual| suppressed_sources.contains(&residual.source()))
}

fn alias_root(
    problem: &Problem,
    variable_id: VariableId,
    suppressed_sources: &[SourceConstraintId],
) -> Result<VariableId, CoreError> {
    let mut current = variable_id;
    let mut path = Vec::new();
    loop {
        if path.contains(&current) {
            return Err(CoreError::AliasCycle {
                variable: variable_id,
            });
        }
        path.push(current);
        let Some(alias) = problem.alias_eliminations.iter().find(|alias| {
            alias.alias == current
                && !declaration_is_suppressed(problem, alias.residual_id, suppressed_sources)
        }) else {
            return Ok(current);
        };
        current = alias.representative;
    }
}

#[allow(clippy::too_many_lines)]
fn structural_summary(
    problem: &Problem,
    incidence: &IncidenceAnalysis,
    active_groups: &[ActiveGroup],
    components: &[SolveComponent],
    eliminated_residuals: &[ResidualId],
) -> Result<StructuralSummary, CoreError> {
    let mut component_summaries = Vec::with_capacity(components.len());
    for component in components {
        let structural_analysis = component_structural_analysis(problem, active_groups, component)?;
        let tangent_dimensions = component
            .variable_ids
            .iter()
            .map(|&id| {
                problem
                    .variables
                    .get(id)
                    .map(|variable| variable.kind().tangent_dimension())
                    .ok_or(CoreError::UnknownVariable(id))
            })
            .sum::<Result<usize, _>>()?;
        let scalar_rows = component
            .residual_ids
            .iter()
            .map(|&id| {
                problem
                    .residuals
                    .get(id)
                    .map(crate::ResidualBlock::output_dimension)
                    .ok_or(CoreError::UnknownResidual(id))
            })
            .sum::<Result<usize, _>>()?;
        let fixed_eliminated_coordinates = problem
            .fixed_eliminations
            .iter()
            .filter(|fixed| component.variable_ids.contains(&fixed.variable_id))
            .map(|fixed| {
                problem
                    .variables
                    .get(fixed.variable_id)
                    .expect("validated fixed variable")
                    .kind()
                    .tangent_dimension()
            })
            .sum();
        let aliased_eliminated_coordinates = problem
            .alias_eliminations
            .iter()
            .filter(|alias| component.variable_ids.contains(&alias.alias))
            .map(|alias| {
                problem
                    .variables
                    .get(alias.alias)
                    .expect("validated alias variable")
                    .kind()
                    .tangent_dimension()
            })
            .sum();
        let eliminated_rows = component
            .residual_ids
            .iter()
            .filter(|id| eliminated_residuals.contains(id))
            .map(|&id| {
                problem
                    .residuals
                    .get(id)
                    .expect("validated eliminated residual")
                    .output_dimension()
            })
            .sum();
        let active_tangent_dimensions = component
            .active_group_indices
            .iter()
            .map(|&index| active_groups[index].kind.tangent_dimension())
            .sum();
        let active_rows: usize = component
            .active_residual_ids
            .iter()
            .map(|&id| {
                problem
                    .residuals
                    .get(id)
                    .expect("active residual belongs to component")
                    .output_dimension()
            })
            .sum();
        if active_tangent_dimensions != structural_analysis.tangent_coordinates.len()
            || active_rows != structural_analysis.rows.len()
        {
            return Err(CoreError::DimensionMismatch {
                context: "component structural envelope",
                expected: active_rows.saturating_add(active_tangent_dimensions),
                actual: structural_analysis
                    .rows
                    .len()
                    .saturating_add(structural_analysis.tangent_coordinates.len()),
            });
        }
        component_summaries.push(ComponentStructuralSummary {
            component_index: component.index,
            pattern_signature: component_signature(
                problem,
                incidence,
                component,
                eliminated_residuals,
            )?,
            sparsity_signature: structural_analysis.sparsity_signature,
            variable_ids: component.variable_ids.clone(),
            residual_ids: component.residual_ids.clone(),
            variable_blocks: component.variable_ids.len(),
            tangent_dimensions,
            residual_blocks: component.residual_ids.len(),
            scalar_rows,
            fixed_eliminated_coordinates,
            aliased_eliminated_coordinates,
            eliminated_rows,
            active_tangent_dimensions,
            active_rows,
            active_hard_rows: active_rows,
            structural_nnz: structural_analysis.structural_nnz,
            structural_rank: structural_analysis.structural_rank,
            structural_left_nullity: structural_analysis.structural_left_nullity,
            structural_right_nullity: structural_analysis.structural_right_nullity,
            structural_classification: structural_analysis.classification,
            dm_partitions: structural_analysis.dm_partitions,
        });
    }

    let structural_rank = component_summaries
        .iter()
        .map(|component| component.structural_rank)
        .sum();
    let structural_left_nullity = component_summaries
        .iter()
        .map(|component| component.structural_left_nullity)
        .sum();
    let structural_right_nullity = component_summaries
        .iter()
        .map(|component| component.structural_right_nullity)
        .sum();
    let mut dm_partitions = DulmageMendelsohnPartitions::default();
    for component in &component_summaries {
        append_dm_partitions(&mut dm_partitions, &component.dm_partitions);
    }
    let sparsity_signature = aggregate_sparsity_signature(&component_summaries);

    Ok(StructuralSummary {
        sparsity_signature,
        variable_blocks: incidence.variable_ids.len(),
        tangent_dimensions: problem
            .variables
            .iter()
            .map(|(_, variable)| variable.kind().tangent_dimension())
            .sum(),
        residual_blocks: incidence.residual_ids.len(),
        scalar_rows: problem
            .residuals
            .iter()
            .map(|(_, residual)| residual.output_dimension())
            .sum(),
        components: components.len(),
        fixed_eliminated_coordinates: problem
            .fixed_eliminations
            .iter()
            .map(|fixed| {
                problem
                    .variables
                    .get(fixed.variable_id)
                    .expect("validated fixed variable")
                    .kind()
                    .tangent_dimension()
            })
            .sum(),
        aliased_eliminated_coordinates: problem
            .alias_eliminations
            .iter()
            .map(|alias| {
                problem
                    .variables
                    .get(alias.alias)
                    .expect("validated alias variable")
                    .kind()
                    .tangent_dimension()
            })
            .sum(),
        eliminated_rows: eliminated_residuals
            .iter()
            .map(|&id| {
                problem
                    .residuals
                    .get(id)
                    .expect("validated eliminated residual")
                    .output_dimension()
            })
            .sum(),
        active_tangent_dimensions: component_summaries
            .iter()
            .map(|component| component.active_tangent_dimensions)
            .sum(),
        active_rows: component_summaries
            .iter()
            .map(|component| component.active_rows)
            .sum(),
        active_hard_rows: component_summaries
            .iter()
            .map(|component| component.active_hard_rows)
            .sum(),
        structural_nnz: component_summaries
            .iter()
            .map(|component| component.structural_nnz)
            .sum(),
        structural_rank,
        structural_left_nullity,
        structural_right_nullity,
        structural_classification: structural_classification(
            structural_left_nullity,
            structural_right_nullity,
        ),
        dm_partitions,
        component_summaries,
    })
}

struct ComponentStructuralAnalysis {
    rows: Vec<ResidualRowRef>,
    tangent_coordinates: Vec<TangentCoordinateRef>,
    sparsity_signature: u64,
    structural_nnz: usize,
    structural_rank: usize,
    structural_left_nullity: usize,
    structural_right_nullity: usize,
    classification: StructuralClassification,
    dm_partitions: DulmageMendelsohnPartitions,
}

fn component_structural_analysis(
    problem: &Problem,
    active_groups: &[ActiveGroup],
    component: &SolveComponent,
) -> Result<ComponentStructuralAnalysis, CoreError> {
    let mut tangent_coordinates = Vec::new();
    let mut group_columns = Vec::new();
    for &group_index in &component.active_group_indices {
        let group = &active_groups[group_index];
        let start = tangent_coordinates.len();
        for coordinate_in_block in 0..group.kind.tangent_dimension() {
            tangent_coordinates.push(TangentCoordinateRef {
                root: group.root,
                coordinate_in_block,
            });
        }
        group_columns.push((group_index, start));
    }

    let mut rows = Vec::new();
    let mut edges = Vec::new();
    for &residual_id in &component.active_residual_ids {
        let residual = problem
            .residuals
            .get(residual_id)
            .ok_or(CoreError::UnknownResidual(residual_id))?;
        for row_in_block in 0..residual.output_dimension() {
            let row = rows.len();
            rows.push(ResidualRowRef {
                residual_id,
                row_in_block,
                source_id: residual.source(),
            });
            let mut row_columns = Vec::new();
            for &variable_id in residual.incident_variables() {
                let Some(&(group_index, column_start)) = group_columns
                    .iter()
                    .find(|&&(index, _)| active_groups[index].members.contains(&variable_id))
                else {
                    // Fixed coordinates remain in canonical evaluation but have
                    // no reduced structural column.
                    continue;
                };
                // ResidualEvaluator declares incidence by variable block, not by
                // scalar formula slot. Keep that entire stable block envelope so
                // matching and sparse patterns never depend on current values.
                for coordinate in 0..active_groups[group_index].kind.tangent_dimension() {
                    row_columns.push(column_start + coordinate);
                }
            }
            row_columns.sort_unstable();
            row_columns.dedup();
            edges.extend(row_columns.into_iter().map(|column| (row, column)));
        }
    }

    let matching = structural_matching(rows.len(), tangent_coordinates.len(), &edges);
    let structural_rank = matching.row_to_column.iter().flatten().count();
    let structural_left_nullity = rows.len().saturating_sub(structural_rank);
    let structural_right_nullity = tangent_coordinates.len().saturating_sub(structural_rank);
    let dm_partitions =
        dulmage_mendelsohn_partitions(&rows, &tangent_coordinates, &matching, &edges);
    Ok(ComponentStructuralAnalysis {
        sparsity_signature: calculate_sparsity_signature(
            rows.len(),
            tangent_coordinates.len(),
            edges.iter().copied(),
        ),
        structural_nnz: edges.len(),
        rows,
        tangent_coordinates,
        structural_rank,
        structural_left_nullity,
        structural_right_nullity,
        classification: structural_classification(
            structural_left_nullity,
            structural_right_nullity,
        ),
        dm_partitions,
    })
}

struct StructuralMatching {
    row_to_column: Vec<Option<usize>>,
    column_to_row: Vec<Option<usize>>,
}

fn structural_matching(
    rows: usize,
    columns: usize,
    edges: &[(usize, usize)],
) -> StructuralMatching {
    // Sorted adjacency plus breadth-first augmenting paths makes the chosen
    // maximum matching deterministic without making DM membership depend on it.
    let adjacency = row_adjacency(rows, edges);
    let mut row_to_column: Vec<Option<usize>> = vec![None; rows];
    let mut column_to_row: Vec<Option<usize>> = vec![None; columns];
    for start_row in 0..rows {
        if row_to_column[start_row].is_some() {
            continue;
        }
        let mut queue = VecDeque::from([start_row]);
        let mut seen_rows = vec![false; rows];
        let mut seen_columns = vec![false; columns];
        let mut parent_column: Vec<Option<usize>> = vec![None; columns];
        seen_rows[start_row] = true;
        let mut free_column = None;
        while let Some(row) = queue.pop_front() {
            for &column in &adjacency[row] {
                if row_to_column[row] == Some(column) || seen_columns[column] {
                    continue;
                }
                seen_columns[column] = true;
                parent_column[column] = Some(row);
                if let Some(matched_row) = column_to_row[column] {
                    if !seen_rows[matched_row] {
                        seen_rows[matched_row] = true;
                        queue.push_back(matched_row);
                    }
                } else {
                    free_column = Some(column);
                    break;
                }
            }
            if free_column.is_some() {
                break;
            }
        }
        let Some(mut column) = free_column else {
            continue;
        };
        loop {
            let row = parent_column[column].expect("augmenting path records each column parent");
            let prior_column = row_to_column[row];
            row_to_column[row] = Some(column);
            column_to_row[column] = Some(row);
            let Some(previous) = prior_column else {
                break;
            };
            column = previous;
        }
    }
    StructuralMatching {
        row_to_column,
        column_to_row,
    }
}

fn dulmage_mendelsohn_partitions(
    rows: &[ResidualRowRef],
    tangent_coordinates: &[TangentCoordinateRef],
    matching: &StructuralMatching,
    edges: &[(usize, usize)],
) -> DulmageMendelsohnPartitions {
    let row_edges = row_adjacency(rows.len(), edges);
    let mut column_edges = vec![Vec::new(); tangent_coordinates.len()];
    for &(row, column) in edges {
        column_edges[column].push(row);
    }

    let mut under_rows = vec![false; rows.len()];
    let mut under_columns = vec![false; tangent_coordinates.len()];
    let mut column_queue = VecDeque::new();
    // Under part: alternating reachability from unmatched tangent coordinates.
    for (column, matched_row) in matching.column_to_row.iter().enumerate() {
        if matched_row.is_none() {
            under_columns[column] = true;
            column_queue.push_back(column);
        }
    }
    while let Some(column) = column_queue.pop_front() {
        for &row in &column_edges[column] {
            if matching.column_to_row[column] == Some(row) || under_rows[row] {
                continue;
            }
            under_rows[row] = true;
            if let Some(matched_column) = matching.row_to_column[row]
                && !under_columns[matched_column]
            {
                under_columns[matched_column] = true;
                column_queue.push_back(matched_column);
            }
        }
    }

    let mut over_rows = vec![false; rows.len()];
    let mut over_columns = vec![false; tangent_coordinates.len()];
    let mut row_queue = VecDeque::new();
    // Over part: alternating reachability from unmatched hard rows.
    for (row, matched_column) in matching.row_to_column.iter().enumerate() {
        if matched_column.is_none() {
            over_rows[row] = true;
            row_queue.push_back(row);
        }
    }
    while let Some(row) = row_queue.pop_front() {
        for &column in &row_edges[row] {
            if matching.row_to_column[row] == Some(column) || over_columns[column] {
                continue;
            }
            over_columns[column] = true;
            if let Some(matched_row) = matching.column_to_row[column]
                && !over_rows[matched_row]
            {
                over_rows[matched_row] = true;
                row_queue.push_back(matched_row);
            }
        }
    }

    debug_assert!((0..rows.len()).all(|row| !(under_rows[row] && over_rows[row])));
    debug_assert!(
        (0..tangent_coordinates.len())
            .all(|column| !(under_columns[column] && over_columns[column]))
    );
    DulmageMendelsohnPartitions {
        under: partition_members(rows, tangent_coordinates, &under_rows, &under_columns),
        well: partition_members(
            rows,
            tangent_coordinates,
            &(0..rows.len())
                .map(|row| !under_rows[row] && !over_rows[row])
                .collect::<Vec<_>>(),
            &(0..tangent_coordinates.len())
                .map(|column| !under_columns[column] && !over_columns[column])
                .collect::<Vec<_>>(),
        ),
        over: partition_members(rows, tangent_coordinates, &over_rows, &over_columns),
    }
}

fn row_adjacency(rows: usize, edges: &[(usize, usize)]) -> Vec<Vec<usize>> {
    let mut adjacency = vec![Vec::new(); rows];
    for &(row, column) in edges {
        adjacency[row].push(column);
    }
    for columns in &mut adjacency {
        columns.sort_unstable();
        columns.dedup();
    }
    adjacency
}

fn partition_members(
    rows: &[ResidualRowRef],
    tangent_coordinates: &[TangentCoordinateRef],
    selected_rows: &[bool],
    selected_columns: &[bool],
) -> DulmageMendelsohnPartition {
    DulmageMendelsohnPartition {
        rows: rows
            .iter()
            .zip(selected_rows)
            .filter_map(|(&row, &selected)| selected.then_some(row))
            .collect(),
        tangent_coordinates: tangent_coordinates
            .iter()
            .zip(selected_columns)
            .filter_map(|(&coordinate, &selected)| selected.then_some(coordinate))
            .collect(),
    }
}

const fn structural_classification(
    left_nullity: usize,
    right_nullity: usize,
) -> StructuralClassification {
    match (left_nullity > 0, right_nullity > 0) {
        (false, true) => StructuralClassification::Under,
        (false, false) => StructuralClassification::Well,
        (true, false) => StructuralClassification::Over,
        (true, true) => StructuralClassification::Mixed,
    }
}

fn append_dm_partitions(
    aggregate: &mut DulmageMendelsohnPartitions,
    component: &DulmageMendelsohnPartitions,
) {
    aggregate.under.rows.extend(&component.under.rows);
    aggregate
        .under
        .tangent_coordinates
        .extend(&component.under.tangent_coordinates);
    aggregate.well.rows.extend(&component.well.rows);
    aggregate
        .well
        .tangent_coordinates
        .extend(&component.well.tangent_coordinates);
    aggregate.over.rows.extend(&component.over.rows);
    aggregate
        .over
        .tangent_coordinates
        .extend(&component.over.tangent_coordinates);
}

fn aggregate_sparsity_signature(components: &[ComponentStructuralSummary]) -> u64 {
    let mut hash = Fnv64::new();
    hash.add_usize(components.len());
    for component in components {
        hash.add_u64(component.sparsity_signature);
    }
    hash.finish()
}

pub(crate) fn calculate_sparsity_signature(
    rows: usize,
    columns: usize,
    edges: impl IntoIterator<Item = (usize, usize)>,
) -> u64 {
    let mut hash = Fnv64::new();
    hash.add_usize(rows);
    hash.add_usize(columns);
    for (row, column) in edges {
        hash.add_usize(row);
        hash.add_usize(column);
    }
    hash.finish()
}

fn component_signature(
    problem: &Problem,
    incidence: &IncidenceAnalysis,
    component: &SolveComponent,
    eliminated_residuals: &[ResidualId],
) -> Result<u64, CoreError> {
    let mut hash = Fnv64::new();
    hash.add_usize(component.variable_ids.len());
    hash.add_usize(component.residual_ids.len());
    for &variable_id in &component.variable_ids {
        let variable = problem
            .variables
            .get(variable_id)
            .ok_or(CoreError::UnknownVariable(variable_id))?;
        hash.add_usize(global_variable_index(incidence, variable_id)?);
        hash.add_u8(kind_code(variable.kind()));
        for scale in variable.step_scales() {
            hash.add_u64(scale.to_bits());
        }
    }
    for &residual_id in &component.residual_ids {
        let residual = problem
            .residuals
            .get(residual_id)
            .ok_or(CoreError::UnknownResidual(residual_id))?;
        hash.add_usize(global_residual_index(incidence, residual_id)?);
        hash.add_u8(category_code(residual.category()));
        hash.add_usize(residual.output_dimension());
        hash.add_u8(u8::from(eliminated_residuals.contains(&residual_id)));
        for &variable_id in residual.incident_variables() {
            hash.add_usize(global_variable_index(incidence, variable_id)?);
        }
        for scale in residual.scales() {
            hash.add_u64(scale.to_bits());
        }
    }
    Ok(hash.finish())
}

fn global_variable_index(
    incidence: &IncidenceAnalysis,
    variable_id: VariableId,
) -> Result<usize, CoreError> {
    incidence
        .variable_ids
        .iter()
        .position(|&id| id == variable_id)
        .ok_or(CoreError::UnknownVariable(variable_id))
}

fn global_residual_index(
    incidence: &IncidenceAnalysis,
    residual_id: ResidualId,
) -> Result<usize, CoreError> {
    incidence
        .residual_ids
        .iter()
        .position(|&id| id == residual_id)
        .ok_or(CoreError::UnknownResidual(residual_id))
}

fn root_for(roots: &[(VariableId, VariableId)], variable_id: VariableId) -> Option<VariableId> {
    roots
        .iter()
        .find_map(|&(variable, root)| (variable == variable_id).then_some(root))
}

pub(crate) fn state_value(state: &VariableState, variable_id: VariableId) -> Option<VariableValue> {
    state
        .values
        .iter()
        .find_map(|&(id, value)| (id == variable_id).then_some(value))
}

pub(crate) fn set_state_value(
    state: &mut VariableState,
    variable_id: VariableId,
    value: VariableValue,
) -> Result<(), CoreError> {
    let (_, current) = state
        .values
        .iter_mut()
        .find(|(id, _)| *id == variable_id)
        .ok_or(CoreError::UnknownVariable(variable_id))?;
    if current.kind() != value.kind() {
        return Err(CoreError::VariableKindMismatch {
            expected: current.kind(),
            actual: value.kind(),
        });
    }
    value.validate_finite()?;
    *current = value;
    Ok(())
}

fn push_unique<T: Copy + PartialEq>(values: &mut Vec<T>, value: T) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn union(parents: &mut [usize], first: usize, second: usize) {
    let first_root = find_root(parents, first);
    let second_root = find_root(parents, second);
    if first_root == second_root {
        return;
    }
    let (lower, higher) = if first_root < second_root {
        (first_root, second_root)
    } else {
        (second_root, first_root)
    };
    parents[higher] = lower;
}

fn find_root(parents: &mut [usize], node: usize) -> usize {
    let mut root = node;
    while parents[root] != root {
        root = parents[root];
    }
    let mut current = node;
    while parents[current] != current {
        let parent = parents[current];
        parents[current] = root;
        current = parent;
    }
    root
}

const fn kind_code(kind: VariableKind) -> u8 {
    match kind {
        VariableKind::Scalar => 0,
        VariableKind::Vec2 => 1,
        VariableKind::Pose2 => 2,
        VariableKind::Vec3 => 3,
        VariableKind::Pose3 => 4,
    }
}

const fn category_code(category: ResidualCategory) -> u8 {
    match category {
        ResidualCategory::Hard => 0,
        ResidualCategory::Temporary => 1,
        ResidualCategory::Preference => 2,
    }
}

struct Fnv64(u64);

impl Fnv64 {
    const fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    fn add_u8(&mut self, value: u8) {
        self.0 ^= u64::from(value);
        self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
    }

    fn add_u64(&mut self, value: u64) {
        for byte in value.to_le_bytes() {
            self.add_u8(byte);
        }
    }

    fn add_usize(&mut self, value: usize) {
        self.add_u64(value as u64);
    }

    const fn finish(self) -> u64 {
        self.0
    }
}
