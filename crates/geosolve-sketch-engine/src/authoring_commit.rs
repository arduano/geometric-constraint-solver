// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared compiler/native preparation and installation behind typed operation handles.
//! Specialized replay and authorization remain with each operation and collaboration owner.

use crate::{AcceptedEvaluation, EditableSession, EngineError, PreparedAuthoringMutation};
use geosolve_constraint_editor::ProjectionalEditorSession;
use geosolve_sketch_code::editor_terminal::{
    PreparedDeclarationLabelProjection, canvas_declaration_label_projections,
    validate_construction_parity,
};
use geosolve_sketch_code::{
    CodeSessionIdentity, EditorBootstrapDeclaration, ManagedSketchMutation,
    PreparedManagedMutationReceipt, SemanticSymbol,
};

#[derive(Debug)]
pub(super) struct PreparedNativeAuthoring {
    pub expected: CodeSessionIdentity,
    pub mutation: PreparedAuthoringMutation,
    pub editor: Box<ProjectionalEditorSession>,
    pub projections: Vec<PreparedDeclarationLabelProjection>,
}

#[derive(Debug)]
pub(super) struct PreparedNativeAuthoringCommit {
    pub expected: CodeSessionIdentity,
    pub candidate: EditableSession,
    pub project: String,
    pub design: crate::EditableDesign,
    pub digest: String,
    pub declarations: Vec<SemanticSymbol>,
}

impl EditableSession {
    /// A remote predictor receives source/design, never private retained history.
    /// Reconstruct that exact public basis only when native replay differs. The
    /// caller must authenticate the complete canonical command before normalizing
    /// native-generated display labels, then compare the complete warm replay.
    pub(super) fn cold_prediction_basis(&self) -> Result<Self, EngineError> {
        Self::open(
            &self.export_project_json()?,
            Some(
                &serde_json::to_string(&self.design())
                    .map_err(|error| EngineError::Admission(error.to_string()))?,
            ),
        )
    }

    pub(super) fn prepare_native_authoring(
        &self,
        editor: Box<ProjectionalEditorSession>,
        declarations: &[EditorBootstrapDeclaration],
        mutation: ManagedSketchMutation,
        high_water: u64,
    ) -> Result<PreparedNativeAuthoring, EngineError> {
        let mutation = self.prepare_managed_mutation(mutation, high_water)?;
        let projections = canvas_declaration_label_projections(&editor, declarations)
            .map_err(EngineError::Admission)?;
        Ok(PreparedNativeAuthoring {
            expected: self.token().clone(),
            mutation,
            editor,
            projections,
        })
    }

    pub(super) fn resolve_native_authoring(
        &self,
        prepared: &PreparedNativeAuthoring,
        receipt: PreparedManagedMutationReceipt,
        operation: &str,
        history_label: &str,
    ) -> Result<PreparedNativeAuthoringCommit, EngineError> {
        if self.token() != &prepared.expected {
            return Err(EngineError::Admission(format!(
                "{operation} preparation is stale or foreign"
            )));
        }
        let mut candidate = self.fork_for_preparation();
        candidate.apply_managed_mutation_for_host(&prepared.mutation, receipt, history_label)?;
        let materialized = &candidate.accepted().0.materialized;
        validate_construction_parity(
            &prepared.editor,
            &materialized.editor,
            &materialized.expansion,
            &prepared.projections,
        )
        .map_err(EngineError::Admission)?;
        Ok(PreparedNativeAuthoringCommit {
            expected: prepared.expected.clone(),
            project: candidate.export_project_json()?,
            design: candidate.design(),
            digest: candidate.source_design_digest()?,
            candidate,
            declarations: prepared
                .projections
                .iter()
                .map(|projection| projection.declaration.clone())
                .collect(),
        })
    }

    pub(super) fn install_native_authoring(
        &mut self,
        prepared: PreparedNativeAuthoringCommit,
    ) -> Result<AcceptedEvaluation, EngineError> {
        self.install_prepared_session(&prepared.expected, prepared.candidate)
    }
}
