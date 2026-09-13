// SPDX-License-Identifier: GPL-3.0-or-later
//! Headless managed compiler transactions over independently accepted engine sessions.
//!
//! These are trusted host APIs, not collaboration wire commands. A collaboration authority
//! must resolve durable target generations, property ownership and deletion dependencies
//! before preparing a mutation. Exact compiler tickets protect the subsequent asynchronous
//! compilation and publication; they do not grant client or per-user publication authority.

use geosolve_sketch_code::{
    CodeInteractionOverlay, CompiledManagedSource, ManagedMutationAuthority, ManagedPresentation,
    ManagedSketchMutation, ManagedStatement, ManagedValue, PreparedManagedMutationReceipt,
    PreparedManagedMutationRequest, PreparedManagedSourceRequest, ProjectKey, SemanticOutputPath,
    SemanticSymbol, derive_managed_value_mutation, prepare_managed_mutation,
    prepare_managed_source, validate_prepared_managed_mutation, validate_prepared_managed_source,
};
use serde::{Deserialize, Serialize};

use crate::{AcceptedEvaluation, EditableSession, EngineAcceptedResult, EngineError};

fn error(value: impl std::fmt::Display) -> EngineError {
    EngineError::Admission(value.to_string())
}

/// An exact accepted-input operation. Only engine preparation can construct this handle.
#[derive(Clone, Debug)]
pub struct PreparedAuthoringMutation {
    request: PreparedManagedMutationRequest,
}

impl PreparedAuthoringMutation {
    pub fn request(&self) -> &PreparedManagedMutationRequest {
        &self.request
    }
}

/// Immutable captured source candidate bound to the exact accepted input.
#[derive(Clone, Debug)]
pub struct PreparedAuthoringSource {
    request: PreparedManagedSourceRequest,
}

impl PreparedAuthoringSource {
    pub fn request(&self) -> &PreparedManagedSourceRequest {
        &self.request
    }
}

/// A semantic input property. Keyed member paths are resolved afresh at admission.
/// The host must authenticate the declaration's durable lifetime before calling the engine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringValueWrite {
    pub declaration: SemanticSymbol,
    pub path: SemanticOutputPath,
    pub value: ManagedValue,
}

#[derive(Clone, Debug)]
struct ValueChange {
    write: AuthoringValueWrite,
    prior: ManagedValue,
}

/// Prepared atomic value batch plus its captured pre-edit values. It is not yet an Undo entry.
#[derive(Clone, Debug)]
pub struct PreparedAuthoringValues {
    mutation: PreparedAuthoringMutation,
    changes: Vec<ValueChange>,
}

impl PreparedAuthoringValues {
    pub fn request(&self) -> &PreparedManagedMutationRequest {
        self.mutation.request()
    }
}

/// Domain-level inverse available only after successful publication.
///
/// This additionally requires the collaboration host's checked property ownership and
/// target-generation guard: equal values alone cannot detect another user's same-value write
/// or deletion/recreation. It deliberately cannot roll back global session history.
#[derive(Clone, Debug)]
pub struct AuthoringValueInverse {
    project: ProjectKey,
    session: u64,
    changes: Vec<ValueChange>,
}

impl AuthoringValueInverse {
    /// Forward writes paired with their previous values, for the host's durable
    /// contribution record. Restoring such a record still requires its own lifecycle
    /// and property-ownership checks; this session-local handle is not restart authority.
    pub fn changes(&self) -> impl ExactSizeIterator<Item = (&AuthoringValueWrite, &ManagedValue)> {
        self.changes
            .iter()
            .map(|change| (&change.write, &change.prior))
    }
}

impl EditableSession {
    fn managed_authority(
        &self,
    ) -> Result<(ManagedMutationAuthority, &CompiledManagedSource), EngineError> {
        let snapshot = self.code_snapshot();
        let project = snapshot
            .code_project
            .as_ref()
            .ok_or_else(|| error("missing managed project"))?;
        let compiled = project
            .managed
            .compiled
            .as_deref()
            .ok_or_else(|| error("missing accepted compiler authority"))?;
        let expansion = snapshot
            .accepted_expansion
            .as_ref()
            .ok_or_else(|| error("missing accepted expansion authority"))?;
        let authority = ManagedMutationAuthority::new(
            project.project.clone(),
            self.token().clone(),
            expansion.digest.clone(),
            project.managed.declaration_name_high_water,
            compiled,
        )
        .map_err(error)?;
        Ok((authority, compiled))
    }

    /// Prepares any existing managed mutation against the current accepted source.
    /// Names and their nondecreasing high-water mark are allocated by the trusted host.
    ///
    /// # Errors
    /// Rejects invalid targets, values, dependencies, compiler authority or allocator changes.
    pub fn prepare_managed_mutation(
        &self,
        mutation: ManagedSketchMutation,
        candidate_name_high_water: u64,
    ) -> Result<PreparedAuthoringMutation, EngineError> {
        let (authority, compiled) = self.managed_authority()?;
        Ok(PreparedAuthoringMutation {
            request: prepare_managed_mutation(
                &authority,
                compiled,
                mutation,
                candidate_name_high_water,
            )
            .map_err(error)?,
        })
    }

    /// Derives parameter identity from the current accepted source namespace.
    /// The host authenticates the source owner's durable generation before calling;
    /// browser-proposed names never participate in this allocation.
    ///
    /// # Errors
    /// Rejects unavailable/nonliteral source values, invalid presentation or exhausted names.
    pub fn prepare_parameter_extraction(
        &self,
        declaration: SemanticSymbol,
        path: SemanticOutputPath,
        presentation: ManagedPresentation,
    ) -> Result<PreparedAuthoringMutation, EngineError> {
        let (authority, compiled) = self.managed_authority()?;
        let mut occupied = std::collections::BTreeSet::new();
        let mut symbols = std::collections::BTreeSet::new();
        let mut binding = None;
        for import in &compiled.ir.imports {
            occupied.extend(import.bindings.iter().map(String::as_str));
        }
        for statement in &compiled.ir.statements {
            match statement {
                ManagedStatement::Declaration {
                    variable, symbol, ..
                } => {
                    occupied.extend([variable.as_str(), symbol.as_str()]);
                    symbols.insert(symbol.as_str());
                }
                ManagedStatement::Binding {
                    variable,
                    parameter,
                    ..
                } => {
                    occupied.insert(variable.as_str());
                    if let Some(parameter) = parameter {
                        occupied.insert(parameter.symbol.as_str());
                        symbols.insert(parameter.symbol.as_str());
                    } else if variable == &declaration.0 && path.0.is_empty() {
                        binding = Some(variable.clone());
                    }
                }
                _ => {}
            }
        }
        let mut high_water = authority.declaration_name_high_water;
        // Promoting a whole scalar binding keeps its existing source identity when
        // possible. New inline bindings reserve a never-decreasing namespace slot.
        let symbol = if let Some(variable) = binding
            .as_ref()
            .filter(|name| !symbols.contains(name.as_str()))
        {
            variable.clone()
        } else {
            loop {
                high_water = high_water
                    .checked_add(1)
                    .filter(|value| *value <= geosolve_sketch_code::MAX_CODE_SESSION_WIRE_INTEGER)
                    .ok_or_else(|| error("managed parameter-name allocator is exhausted"))?;
                let candidate = format!("parameter{high_water}");
                if !occupied.contains(candidate.as_str()) {
                    break candidate;
                }
            }
        };
        self.prepare_managed_mutation(
            ManagedSketchMutation::ExtractParameter {
                declaration: declaration.0,
                path: path.0,
                variable: binding.unwrap_or_else(|| symbol.clone()),
                symbol,
                presentation,
            },
            high_water,
        )
    }

    /// Authenticates the live accepted input and compiler receipt, then publishes atomically
    /// through the existing source reconciliation and independent native residual validation.
    ///
    /// # Errors
    /// Replayed, stale, forged, unrelated or geometrically rejected candidates retain all state.
    pub fn apply_managed_mutation(
        &mut self,
        prepared: &PreparedAuthoringMutation,
        receipt: PreparedManagedMutationReceipt,
    ) -> Result<AcceptedEvaluation, EngineError> {
        self.apply_managed_mutation_for_host(prepared, receipt, "Apply project")
    }

    pub(super) fn apply_managed_mutation_for_host(
        &mut self,
        prepared: &PreparedAuthoringMutation,
        receipt: PreparedManagedMutationReceipt,
        history_label: &str,
    ) -> Result<AcceptedEvaluation, EngineError> {
        let (authority, _) = self.managed_authority()?;
        let validated = validate_prepared_managed_mutation(&authority, &prepared.request, receipt)
            .map_err(error)?;
        let high_water = validated.declaration_name_high_water();
        self.apply_managed_compilation(validated.into_compiled(), high_water, history_label)
    }

    /// Captures an explicit Apply candidate. Later draft typing cannot alter these bytes.
    /// The host must first reconcile this captured draft with intervening accepted edits.
    ///
    /// # Errors
    /// Rejects unchanged/oversized source or invalid accepted compiler authority.
    pub fn prepare_managed_source(
        &self,
        source: String,
    ) -> Result<PreparedAuthoringSource, EngineError> {
        let (authority, compiled) = self.managed_authority()?;
        Ok(PreparedAuthoringSource {
            request: prepare_managed_source(&authority, compiled, source).map_err(error)?,
        })
    }

    /// Publishes exactly the captured source after compiler and native validation.
    ///
    /// # Errors
    /// Stale, unrelated or rejected receipts preserve accepted source, geometry and history.
    pub fn apply_managed_source(
        &mut self,
        prepared: &PreparedAuthoringSource,
        receipt: PreparedManagedMutationReceipt,
    ) -> Result<AcceptedEvaluation, EngineError> {
        let (authority, _) = self.managed_authority()?;
        let validated = validate_prepared_managed_source(&authority, &prepared.request, receipt)
            .map_err(error)?;
        let high_water = validated.declaration_name_high_water();
        self.apply_managed_compilation(validated.into_compiled(), high_water, "Apply project")
    }

    fn apply_managed_compilation(
        &mut self,
        compiled: CompiledManagedSource,
        high_water: u64,
        history_label: &str,
    ) -> Result<AcceptedEvaluation, EngineError> {
        let mut project = self
            .code_snapshot()
            .code_project
            .clone()
            .ok_or_else(|| error("missing managed project"))?;
        project.managed = compiled.into_managed_document().map_err(error)?;
        project.managed.declaration_name_high_water = high_water;
        project.validate().map_err(error)?;
        self.apply_project_for_host(project, history_label, false)?;
        Ok(self.accepted().clone())
    }

    /// Resolves current lexical expectations for one atomic semantic property batch.
    /// Incoming values use latest-write semantics; no stale client value becomes a CAS basis.
    ///
    /// # Errors
    /// Rejects absent/ambiguous paths, invalid/overlapping writes or unsupported expressions.
    pub fn prepare_managed_values(
        &self,
        writes: Vec<AuthoringValueWrite>,
    ) -> Result<PreparedAuthoringValues, EngineError> {
        if writes.len() > geosolve_sketch_code::MANAGED_MUTATION_BATCH_LIMIT {
            return Err(error("managed property batch exceeds the mutation limit"));
        }
        let (authority, compiled) = self.managed_authority()?;
        let mut values = Vec::with_capacity(writes.len());
        let mut changes = Vec::with_capacity(writes.len());
        for write in writes {
            let value = derive_managed_value_mutation(
                compiled,
                &write.declaration,
                &write.path,
                write.value.clone(),
            )
            .map_err(error)?;
            changes.push(ValueChange {
                write,
                prior: value.expected.clone(),
            });
            values.push(value);
        }
        let mutation = self.prepare_managed_mutation(
            ManagedSketchMutation::SetValues { values },
            authority.declaration_name_high_water,
        )?;
        Ok(PreparedAuthoringValues { mutation, changes })
    }

    /// Publishes an atomic property batch and returns its domain checked-inverse data.
    ///
    /// # Errors
    /// Exact-input/compiler/native failures retain all accepted state and issue no inverse.
    pub fn apply_managed_values(
        &mut self,
        prepared: &PreparedAuthoringValues,
        receipt: PreparedManagedMutationReceipt,
    ) -> Result<(AcceptedEvaluation, AuthoringValueInverse), EngineError> {
        let accepted = self.apply_managed_mutation(&prepared.mutation, receipt)?;
        let inverse = AuthoringValueInverse {
            project: prepared.request().ticket.project.clone(),
            session: self.token().session,
            changes: prepared.changes.clone(),
        };
        Ok((accepted, inverse))
    }

    /// Prepares the inverse of this exact value contribution against current semantic paths.
    /// The host must also check user ownership and durable target generations before calling.
    /// Applying the returned batch yields inverse data suitable for the same guarded Redo path.
    ///
    /// # Errors
    /// Rejects foreign sessions, missing targets or any property no longer equal to its forward value.
    pub fn prepare_managed_value_inverse(
        &self,
        inverse: &AuthoringValueInverse,
    ) -> Result<PreparedAuthoringValues, EngineError> {
        let (authority, compiled) = self.managed_authority()?;
        if authority.project != inverse.project || self.token().session != inverse.session {
            return Err(error("value inverse belongs to a different session"));
        }
        let mut writes = Vec::with_capacity(inverse.changes.len());
        for change in &inverse.changes {
            let current = derive_managed_value_mutation(
                compiled,
                &change.write.declaration,
                &change.write.path,
                change.prior.clone(),
            )
            .map_err(error)?;
            if current.expected != change.write.value {
                return Err(error(
                    "value inverse would overwrite a newer property value",
                ));
            }
            writes.push(AuthoringValueWrite {
                declaration: change.write.declaration.clone(),
                path: change.write.path.clone(),
                value: change.prior.clone(),
            });
        }
        self.prepare_managed_values(writes)
    }
}

/// Isolated client prediction using the same compiler/domain semantics as accepted editing.
/// Only provisional report DTOs escape; this type cannot install a result in a server session.
/// Ordered gesture interpretation still belongs to the shared retained coordinator.
#[derive(Debug)]
pub struct AuthoringPrediction {
    session: EditableSession,
}

impl AuthoringPrediction {
    /// # Errors
    /// Rejects invalid source/design or geometry through ordinary independent engine validation.
    pub fn open(project_json: &str, design_json: Option<&str>) -> Result<Self, EngineError> {
        Ok(Self {
            session: EditableSession::open(project_json, design_json)?,
        })
    }

    pub fn result(&self) -> &EngineAcceptedResult {
        self.session.accepted().result()
    }

    /// # Errors
    /// Rejects the same unsupported targets/values as accepted managed preparation.
    pub fn prepare_managed_values(
        &self,
        writes: Vec<AuthoringValueWrite>,
    ) -> Result<PreparedAuthoringValues, EngineError> {
        self.session.prepare_managed_values(writes)
    }

    /// # Errors
    /// Rejects invalid/stale compilation or geometry, retaining the last provisional report.
    pub fn apply_managed_values(
        &mut self,
        prepared: &PreparedAuthoringValues,
        receipt: PreparedManagedMutationReceipt,
    ) -> Result<&EngineAcceptedResult, EngineError> {
        self.session.apply_managed_values(prepared, receipt)?;
        Ok(self.result())
    }

    /// Predicts shared semantic point/branch overrides without granting publication authority.
    ///
    /// # Errors
    /// Invalid semantic addresses, branches or rejected geometry preserve the prior prediction.
    pub fn apply_overlay(
        &mut self,
        overlay: CodeInteractionOverlay,
    ) -> Result<&EngineAcceptedResult, EngineError> {
        let expected = self.session.token().clone();
        self.session.apply_overlay(&expected, overlay)?;
        Ok(self.result())
    }
}
