// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded, presentation-only accepted-scene transport.
//!
//! This codec carries evaluated geometry and presentation inputs. It never
//! restores a retained session, solves a document, evaluates a computed feature,
//! or deserializes prepared-input/publication authority.

use serde::{Deserialize, Serialize};

use super::{
    BTreeSet, ComputedConstructionFragmentId, ComputedConstructionFragmentProvenance,
    ComputedCornerRef, ComputedEdgeId, ComputedEvaluationRevision, ComputedFeatureCornerId,
    ComputedFeatureId, ComputedFilletContact, ComputedSourceInterval, ConstraintEditor,
    ContactDomain, CurveSpan, DesignPointId, DocumentArcSweep, EditorEffect, EditorScene,
    EditorTool, GeometryRole, Modifiers, NativeCurveSpanSource, PointerInput,
    ResolvedSelectPointerTarget, SceneAnnotation, SceneAnnotationGeometry, SceneComputedCurve,
    SceneConstraintEntry, SceneCurve, SceneCurveOrigin, SceneFilletAction,
    SceneFilletActionAvailability, SceneFilletActionControlGeometry, SceneFilletActionId,
    SceneFilletAlternativeGeometry, SceneFilletCornerAffordances, SceneFilletRadiusRail,
    ScenePoint, ScreenPoint, SelectionItem, SketchDesignIdentity, SketchDocument, Viewport,
    is_linear_span, model_positions_bit_equal, painted_contact_domain, scene_datums,
    screen_points_match, tessellate_scene_computed_arc,
};

const FORMAT: &str = "geosolve-detached-scene-v1";
const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_ITEMS: usize = 100_000;
const MAX_SAMPLES: usize = 1_000_000;

/// A malformed or oversized detached presentation cannot replace a live scene.
#[derive(Debug, thiserror::Error)]
pub enum DetachedSceneError {
    #[error("detached scene exceeds its bounded transport limits")]
    ResourceLimit,
    #[error("invalid detached scene: {0}")]
    Invalid(String),
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "map_err consumes the owned domain error at this transport boundary"
)]
fn invalid(message: impl ToString) -> DetachedSceneError {
    DetachedSceneError::Invalid(message.to_string())
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireScene {
    format: String,
    accepted_revision: u64,
    design_revision: u64,
    document: String,
    viewport: Viewport,
    chord_tolerance_pixels: f64,
    hidden: BTreeSet<SelectionItem>,
    selected_control_owner: Option<CurveSpan>,
    points: Vec<ScenePoint>,
    curves: Vec<WireCurve>,
    computed: Vec<WireComputed>,
    affordances: Vec<WireAffordance>,
    annotations: Vec<SceneAnnotation>,
    constraint_entries: Vec<SceneConstraintEntry>,
    annotations_visible: bool,
    show_all_constraint_annotations: bool,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Corner {
    feature: ComputedFeatureId,
    corner: ComputedFeatureCornerId,
}

impl From<ComputedCornerRef> for Corner {
    fn from(owner: ComputedCornerRef) -> Self {
        Self {
            feature: owner.feature,
            corner: owner.corner,
        }
    }
}

impl From<Corner> for ComputedCornerRef {
    fn from(owner: Corner) -> Self {
        Self {
            feature: owner.feature,
            corner: owner.corner,
        }
    }
}

pub(super) mod corner_codec {
    use super::{ComputedCornerRef, Corner, Deserialize, Serialize};
    pub(crate) fn serialize<S: serde::Serializer>(
        owner: &ComputedCornerRef,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        Corner::from(*owner).serialize(serializer)
    }
    pub(crate) fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<ComputedCornerRef, D::Error> {
        Corner::deserialize(deserializer).map(ComputedCornerRef::from)
    }
}

pub(super) mod document_codec {
    use super::{Deserialize, Serialize, SketchDocument};
    pub(crate) fn serialize<S: serde::Serializer>(
        document: &SketchDocument,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        document
            .to_draft_v5_json()
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
    pub(crate) fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<SketchDocument, D::Error> {
        SketchDocument::from_draft_v5_json(&String::deserialize(deserializer)?)
            .map_err(serde::de::Error::custom)
    }
}

pub(super) mod map_entries {
    use super::{Deserialize, MAX_ITEMS, Serialize};
    use std::collections::BTreeMap;
    pub(crate) fn serialize<K: Serialize, V: Serialize, S: serde::Serializer>(
        map: &BTreeMap<K, V>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        map.iter().collect::<Vec<_>>().serialize(serializer)
    }
    pub(crate) fn deserialize<
        'de,
        K: Deserialize<'de> + Ord,
        V: Deserialize<'de>,
        D: serde::Deserializer<'de>,
    >(
        deserializer: D,
    ) -> Result<BTreeMap<K, V>, D::Error> {
        let rows = Vec::<(K, V)>::deserialize(deserializer)?;
        if rows.len() > MAX_ITEMS {
            return Err(serde::de::Error::custom("excessive presentation entries"));
        }
        let count = rows.len();
        let map = rows.into_iter().collect::<BTreeMap<_, _>>();
        if map.len() != count {
            return Err(serde::de::Error::custom("duplicate presentation key"));
        }
        Ok(map)
    }
}

#[derive(Serialize, Deserialize)]
enum Selected {
    Point(DesignPointId),
    Curve(CurveSpan),
    Constraint(geosolve_sketch::DocumentConstraintId),
    Dimension(geosolve_sketch::DocumentDimensionId),
    Datum(geosolve_sketch::SketchDatum),
    Feature(ComputedFeatureId),
    FeatureCorner(Corner),
}

impl Serialize for SelectionItem {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match *self {
            Self::Point(id) => Selected::Point(id),
            Self::Curve(span) => Selected::Curve(span),
            Self::Constraint(id) => Selected::Constraint(id),
            Self::Dimension(id) => Selected::Dimension(id),
            Self::Datum(datum) => Selected::Datum(datum),
            Self::Feature(id) => Selected::Feature(id),
            Self::FeatureCorner(owner) => Selected::FeatureCorner(owner.into()),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SelectionItem {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match Selected::deserialize(deserializer)? {
            Selected::Point(id) => Self::Point(id),
            Selected::Curve(span) => Self::Curve(span),
            Selected::Constraint(id) => Self::Constraint(id),
            Selected::Dimension(id) => Self::Dimension(id),
            Selected::Datum(datum) => Self::Datum(datum),
            Selected::Feature(id) => Self::Feature(id),
            Selected::FeatureCorner(owner) => Self::FeatureCorner(owner.into()),
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireDiscarded {
    evaluation: ComputedEvaluationRevision,
    ordinal: u32,
    source: NativeCurveSpanSource,
    interval: [f64; 2],
    owner: Corner,
    endpoint: geosolve_sketch::DocumentFilletTrimEndpoint,
    base_interval: [f64; 2],
}

pub(super) mod origin_codec {
    use super::{
        ComputedConstructionFragmentId, ComputedConstructionFragmentProvenance,
        ComputedSourceInterval, Deserialize, SceneCurveOrigin, Serialize, WireDiscarded,
    };

    pub(crate) fn serialize<S: serde::Serializer>(
        origin: &SceneCurveOrigin,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match *origin {
            SceneCurveOrigin::Native => None,
            SceneCurveOrigin::FilletDiscarded {
                fragment,
                source,
                interval,
                provenance,
            } => Some(WireDiscarded {
                evaluation: fragment.evaluation,
                ordinal: fragment.ordinal,
                source,
                interval: [interval.start, interval.end],
                owner: provenance.owner.into(),
                endpoint: provenance.endpoint,
                base_interval: [provenance.base_interval.start, provenance.base_interval.end],
            }),
        }
        .serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<SceneCurveOrigin, D::Error> {
        Ok(Option::<WireDiscarded>::deserialize(deserializer)?.map_or(
            SceneCurveOrigin::Native,
            |discarded| SceneCurveOrigin::FilletDiscarded {
                fragment: ComputedConstructionFragmentId {
                    evaluation: discarded.evaluation,
                    ordinal: discarded.ordinal,
                },
                source: discarded.source,
                interval: ComputedSourceInterval {
                    start: discarded.interval[0],
                    end: discarded.interval[1],
                },
                provenance: ComputedConstructionFragmentProvenance {
                    owner: discarded.owner.into(),
                    endpoint: discarded.endpoint,
                    base_interval: ComputedSourceInterval {
                        start: discarded.base_interval[0],
                        end: discarded.base_interval[1],
                    },
                },
            },
        ))
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireCurve {
    span: CurveSpan,
    authoring_eligible: bool,
    affine: bool,
    contact_domain: ContactDomain,
    role: GeometryRole,
    source_role: GeometryRole,
    discarded: Option<WireDiscarded>,
    screen_polyline: Vec<ScreenPoint>,
    screen_parameters: Vec<f64>,
    drag_handle_point: Option<DesignPointId>,
}

impl From<&SceneCurve> for WireCurve {
    fn from(curve: &SceneCurve) -> Self {
        let discarded = match curve.origin {
            SceneCurveOrigin::Native => None,
            SceneCurveOrigin::FilletDiscarded {
                fragment,
                source,
                interval,
                provenance,
            } => Some(WireDiscarded {
                evaluation: fragment.evaluation,
                ordinal: fragment.ordinal,
                source,
                interval: [interval.start, interval.end],
                owner: provenance.owner.into(),
                endpoint: provenance.endpoint,
                base_interval: [provenance.base_interval.start, provenance.base_interval.end],
            }),
        };
        Self {
            span: curve.span,
            authoring_eligible: curve.authoring_eligible,
            affine: curve.affine,
            contact_domain: curve.contact_domain,
            role: curve.role,
            source_role: curve.source_role,
            discarded,
            screen_polyline: curve.screen_polyline.clone(),
            screen_parameters: curve.screen_parameters.clone(),
            drag_handle_point: curve.drag_handle_point,
        }
    }
}

impl WireCurve {
    fn decode(
        self,
        document: &SketchDocument,
        viewport: Viewport,
    ) -> Result<SceneCurve, DetachedSceneError> {
        if self.screen_polyline.len() < 2
            || self.screen_polyline.len() != self.screen_parameters.len()
            || self
                .screen_parameters
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .drag_handle_point
                .is_some_and(|point| document.point(point).is_none())
        {
            return Err(invalid("malformed native curve samples"));
        }
        for (screen, parameter) in self.screen_polyline.iter().zip(&self.screen_parameters) {
            let point = document
                .evaluate_curve_jet(self.span, *parameter)
                .map_err(invalid)?
                .position;
            if !screen_points_match(viewport.model_to_screen([point.x, point.y]), *screen) {
                return Err(invalid("native sample differs from document geometry"));
            }
        }
        if self.affine != is_linear_span(document, self.span)
            || self.contact_domain
                != painted_contact_domain(document, self.span).map_err(invalid)?
        {
            return Err(invalid("native curve semantics differ from its document"));
        }
        if let Some(discarded) = &self.discarded
            && (discarded.source.span != self.span
                || discarded.interval[1] <= discarded.interval[0]
                || discarded.base_interval[1] <= discarded.base_interval[0])
        {
            return Err(invalid("malformed discarded interval"));
        }
        let origin = self
            .discarded
            .map_or(SceneCurveOrigin::Native, |discarded| {
                SceneCurveOrigin::FilletDiscarded {
                    fragment: ComputedConstructionFragmentId {
                        evaluation: discarded.evaluation,
                        ordinal: discarded.ordinal,
                    },
                    source: discarded.source,
                    interval: ComputedSourceInterval {
                        start: discarded.interval[0],
                        end: discarded.interval[1],
                    },
                    provenance: ComputedConstructionFragmentProvenance {
                        owner: discarded.owner.into(),
                        endpoint: discarded.endpoint,
                        base_interval: ComputedSourceInterval {
                            start: discarded.base_interval[0],
                            end: discarded.base_interval[1],
                        },
                    },
                }
            });
        Ok(SceneCurve {
            span: self.span,
            authoring_eligible: self.authoring_eligible,
            affine: self.affine,
            contact_domain: self.contact_domain,
            role: self.role,
            source_role: self.source_role,
            origin,
            screen_polyline: self.screen_polyline,
            screen_parameters: self.screen_parameters,
            drag_handle_point: self.drag_handle_point,
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireContact {
    source: NativeCurveSpanSource,
    parameter: f64,
    winding: i32,
    total_parameter: f64,
    position: [f64; 2],
}

impl From<ComputedFilletContact> for WireContact {
    fn from(contact: ComputedFilletContact) -> Self {
        Self {
            source: contact.source,
            parameter: contact.parameter,
            winding: contact.winding,
            total_parameter: contact.total_parameter,
            position: contact.position,
        }
    }
}

impl From<WireContact> for ComputedFilletContact {
    fn from(contact: WireContact) -> Self {
        Self {
            source: contact.source,
            parameter: contact.parameter,
            winding: contact.winding,
            total_parameter: contact.total_parameter,
            position: contact.position,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireComputed {
    evaluation: ComputedEvaluationRevision,
    ordinal: u32,
    owner: Corner,
    role: GeometryRole,
    center: [f64; 2],
    radius: f64,
    start_angle: f64,
    end_angle: f64,
    sweep: DocumentArcSweep,
    contacts: [WireContact; 2],
    screen_polyline: Vec<ScreenPoint>,
    radius_rail: Option<SceneFilletRadiusRail>,
}

impl From<&SceneComputedCurve> for WireComputed {
    fn from(curve: &SceneComputedCurve) -> Self {
        Self {
            evaluation: curve.edge.evaluation,
            ordinal: curve.edge.ordinal,
            owner: curve.owner.into(),
            role: curve.role,
            center: curve.center,
            radius: curve.radius,
            start_angle: curve.start_angle,
            end_angle: curve.end_angle,
            sweep: curve.sweep,
            contacts: curve.contacts.map(WireContact::from),
            screen_polyline: curve.screen_polyline.clone(),
            radius_rail: curve.radius_rail,
        }
    }
}

impl WireComputed {
    fn decode(
        self,
        viewport: Viewport,
        tolerance: f64,
    ) -> Result<SceneComputedCurve, DetachedSceneError> {
        let curve = SceneComputedCurve {
            edge: ComputedEdgeId {
                evaluation: self.evaluation,
                ordinal: self.ordinal,
            },
            owner: self.owner.into(),
            role: self.role,
            center: self.center,
            radius: self.radius,
            start_angle: self.start_angle,
            end_angle: self.end_angle,
            sweep: self.sweep,
            contacts: self.contacts.map(ComputedFilletContact::from),
            screen_polyline: self.screen_polyline,
            radius_rail: self.radius_rail,
        };
        let expected =
            tessellate_scene_computed_arc(&curve, viewport, tolerance).map_err(invalid)?;
        if expected != curve.screen_polyline
            || curve.radius_rail.is_some_and(|rail| !rail.is_valid())
            || curve.contacts.iter().any(|contact| {
                !contact.parameter.is_finite()
                    || !contact.total_parameter.is_finite()
                    || !contact.position.into_iter().all(f64::is_finite)
            })
        {
            return Err(invalid("computed arc differs from its model geometry"));
        }
        Ok(curve)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireAction {
    id: SceneFilletActionId,
    owner: Corner,
    label: String,
    availability: SceneFilletActionAvailability,
    control_geometry: Option<SceneFilletActionControlGeometry>,
    dashed_alternative_arc: Option<SceneFilletAlternativeGeometry>,
}

impl From<&SceneFilletAction> for WireAction {
    fn from(action: &SceneFilletAction) -> Self {
        Self {
            id: action.id,
            owner: action.owner.into(),
            label: action.label.clone(),
            availability: action.availability.clone(),
            control_geometry: action.control_geometry,
            dashed_alternative_arc: action.dashed_alternative_arc.clone(),
        }
    }
}

impl From<WireAction> for SceneFilletAction {
    fn from(action: WireAction) -> Self {
        Self {
            id: action.id,
            owner: action.owner.into(),
            label: action.label,
            availability: action.availability,
            control_geometry: action.control_geometry,
            dashed_alternative_arc: action.dashed_alternative_arc,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireAffordance {
    owner: Corner,
    affected_owners: Vec<Corner>,
    derivative: [f64; 2],
    actions: Vec<WireAction>,
}

impl From<&SceneFilletCornerAffordances> for WireAffordance {
    fn from(affordance: &SceneFilletCornerAffordances) -> Self {
        Self {
            owner: affordance.owner.into(),
            affected_owners: affordance
                .affected_owners
                .iter()
                .copied()
                .map(Corner::from)
                .collect(),
            derivative: affordance.radius_rail.model_derivative,
            actions: affordance.actions.iter().map(WireAction::from).collect(),
        }
    }
}

impl EditorScene {
    /// Exports complete presentation geometry without retained edit authority.
    ///
    /// # Errors
    /// Rejects caller-mutated, malformed, non-finite or oversized scenes.
    pub fn to_detached_json(&self) -> Result<String, DetachedSceneError> {
        if !self.retained_reprojection_semantics_are_sealed() {
            return Err(invalid("scene presentation seal is stale"));
        }
        let wire = WireScene {
            format: FORMAT.into(),
            accepted_revision: self.accepted_revision,
            design_revision: self.design_identity.revision().get(),
            document: self.accepted_document.to_draft_v5_json().map_err(invalid)?,
            viewport: self.viewport,
            chord_tolerance_pixels: self.chord_tolerance_pixels,
            hidden: self.hidden_presentation_items.clone(),
            selected_control_owner: self.selected_curve_control_owner,
            points: self.points.clone(),
            curves: self.curves.iter().map(WireCurve::from).collect(),
            computed: self
                .computed_curves
                .iter()
                .map(WireComputed::from)
                .collect(),
            affordances: self
                .fillet_affordances
                .iter()
                .map(WireAffordance::from)
                .collect(),
            annotations: self.annotations.clone(),
            constraint_entries: self.constraint_entries.clone(),
            annotations_visible: self.annotations_visible,
            show_all_constraint_annotations: self.show_all_constraint_annotations,
        };
        wire.validate_limits()?;
        // A typed round trip detects serde_json's non-finite-float -> null rule.
        let json = serde_json::to_string(&wire).map_err(invalid)?;
        if json.len() > MAX_BYTES {
            return Err(DetachedSceneError::ResourceLimit);
        }
        let _: WireScene = serde_json::from_str(&json).map_err(invalid)?;
        Ok(json)
    }

    /// Imports detached geometry for local camera, picking and presentation.
    ///
    /// The result is permanently ineligible for retained-session publication.
    /// No solver or feature evaluator runs, including for the initial document.
    ///
    /// # Errors
    /// Rejects invalid versions, unknown fields, malformed geometry and limits.
    #[allow(
        clippy::too_many_lines,
        reason = "one atomic decoder explicitly enumerates all transported geometry and excluded authority"
    )]
    pub fn from_detached_json(json: &str) -> Result<Self, DetachedSceneError> {
        if json.len() > MAX_BYTES {
            return Err(DetachedSceneError::ResourceLimit);
        }
        let wire: WireScene = serde_json::from_str(json).map_err(invalid)?;
        wire.validate_limits()?;
        if wire.format != FORMAT {
            return Err(invalid("unsupported format"));
        }
        if !wire.viewport.is_valid()
            || !wire.chord_tolerance_pixels.is_finite()
            || wire.chord_tolerance_pixels <= 0.0
        {
            return Err(invalid("invalid viewport or curve tolerance"));
        }
        let document = SketchDocument::from_draft_v5_json(&wire.document).map_err(invalid)?;
        let design_identity =
            SketchDesignIdentity::for_detached_presentation(document.id(), wire.design_revision);
        if wire
            .points
            .iter()
            .map(|point| point.id)
            .collect::<BTreeSet<_>>()
            .len()
            != wire.points.len()
            || wire
                .annotations
                .iter()
                .map(|annotation| (annotation.item, annotation.source))
                .collect::<BTreeSet<_>>()
                .len()
                != wire.annotations.len()
        {
            return Err(invalid("duplicate point or annotation identity"));
        }
        for annotation in &wire.annotations {
            validate_annotation(annotation, &document)?;
        }
        for point in &wire.points {
            if document.point(point.id).is_none_or(|native| {
                !model_positions_bit_equal(native.position, point.model_position)
            }) || !point.screen_position.is_finite()
                || !screen_points_match(
                    wire.viewport.model_to_screen(point.model_position),
                    point.screen_position,
                )
            {
                return Err(invalid("point differs from document geometry"));
            }
        }
        let curves = wire
            .curves
            .into_iter()
            .map(|curve| curve.decode(&document, wire.viewport))
            .collect::<Result<_, _>>()?;
        let computed_curves = wire
            .computed
            .into_iter()
            .map(|curve| curve.decode(wire.viewport, wire.chord_tolerance_pixels))
            .collect::<Result<_, _>>()?;
        // Retain the exact transported presentation instead of cold-building
        // every native curve and annotation a second time. The bounded samples
        // above were checked with the public accepted-domain evaluator.
        let mut scene = Self {
            detached_transport: true,
            accepted_revision: wire.accepted_revision,
            design_identity,
            prepared_input: None,
            draft_inference_seal: None,
            retained_reprojection_seal: None,
            hidden_presentation_items: wire.hidden,
            chord_tolerance_pixels: wire.chord_tolerance_pixels,
            selected_curve_control_owner: None,
            viewport: wire.viewport,
            points: wire.points,
            curves,
            datums: scene_datums(wire.viewport),
            computed_curves,
            curve_controls: Vec::new(),
            curve_control_guides: Vec::new(),
            feature_identity: None,
            computed_input: None,
            fillet_interaction_origin: None,
            offset_distance_interaction_origin: None,
            accepted_offset_distance_interaction_origin: None,
            curve_control_interaction_origin: None,
            fillet_affordances: Vec::new(),
            computed_fillet_continuation_statuses: Vec::new(),
            annotations: wire.annotations,
            annotations_visible: wire.annotations_visible,
            show_all_constraint_annotations: wire.show_all_constraint_annotations,
            constraint_entries: wire.constraint_entries,
            construction_snap_points: Vec::new(),
            accepted_document: document,
        };
        for affordance in wire.affordances {
            let owner = affordance.owner.into();
            let curve = scene
                .computed_curves
                .iter()
                .find(|curve| curve.owner == owner)
                .ok_or_else(|| invalid("affordance has no computed owner"))?;
            let (radius_rail, contacts) = Self::derive_computed_fillet_radius_affordances(
                curve,
                wire.viewport,
                owner,
                affordance.derivative,
            )
            .map_err(invalid)?;
            let actions = affordance
                .actions
                .into_iter()
                .map(SceneFilletAction::from)
                .collect::<Vec<_>>();
            if actions
                .iter()
                .any(|action| !action.is_valid(owner, wire.viewport))
            {
                return Err(invalid("malformed computed action presentation"));
            }
            scene.fillet_affordances.push(SceneFilletCornerAffordances {
                owner,
                affected_owners: affordance
                    .affected_owners
                    .into_iter()
                    .map(ComputedCornerRef::from)
                    .collect(),
                radius_rail,
                contacts,
                actions,
                continuation_status: None,
            });
        }
        scene.datums.retain(|datum| {
            !scene
                .hidden_presentation_items
                .contains(&SelectionItem::Datum(datum.datum))
        });
        scene
            .set_selected_curve_controls(wire.selected_control_owner)
            .map_err(invalid)?;
        scene.refresh_retained_reprojection_seal();
        Ok(scene)
    }

    /// Atomically replaces a detached presentation after complete decoding.
    ///
    /// # Errors
    /// A decode failure preserves every value of the previous scene.
    pub fn replace_detached_json(&mut self, json: &str) -> Result<(), DetachedSceneError> {
        let replacement = Self::from_detached_json(json)?;
        *self = replacement;
        Ok(())
    }

    /// Immutable geometry document used by detached annotation rendering.
    /// This is data, never an independently accepted session or edit capability.
    #[must_use]
    pub const fn presentation_document(&self) -> &SketchDocument {
        &self.accepted_document
    }

    /// Whether the scene came from the presentation-only transport boundary.
    #[must_use]
    pub const fn is_detached_presentation(&self) -> bool {
        self.detached_transport
    }
}

impl ConstraintEditor {
    /// Applies the ordinary Select-mode hit resolver without beginning an edit.
    ///
    /// This is the local selection boundary for hosts whose geometry edits are
    /// remote. Curve parameter/origin, modifier toggles and annotation priority
    /// are exactly the ordinary pointer-down contract. Existing authoring or
    /// active gestures are never interrupted by this method.
    pub fn select_at(&mut self, scene: &EditorScene, input: PointerInput) -> Vec<EditorEffect> {
        if self.tool != EditorTool::Select
            || self.active_pointer_gesture().is_some()
            || !input.position.is_finite()
        {
            return Vec::new();
        }
        let target = self.resolve_select_pointer_target(scene, input.position, &[]);
        // A grip belongs to the already selected curve; ordinary pointer-down
        // starts its edit without replacing the selected curve or picked span.
        if matches!(target, Some(ResolvedSelectPointerTarget::CurveControl(_))) {
            return self.clear_fillet_branch_preview();
        }
        let resolved_input = if matches!(
            target,
            Some(
                ResolvedSelectPointerTarget::FilletRadius(_)
                    | ResolvedSelectPointerTarget::CurveControl(_)
            )
        ) {
            PointerInput {
                modifiers: Modifiers::default(),
                ..input
            }
        } else {
            input
        };
        let effects = self.pointer_down_resolved_hit(
            scene,
            resolved_input,
            target.map(ResolvedSelectPointerTarget::hit),
        );
        self.point_gesture = None;
        self.feature_radius_gesture = None;
        effects
    }
}

impl WireScene {
    fn validate_limits(&self) -> Result<(), DetachedSceneError> {
        let count = self
            .points
            .len()
            .saturating_add(self.curves.len())
            .saturating_add(self.computed.len())
            .saturating_add(self.affordances.len())
            .saturating_add(self.annotations.len())
            .saturating_add(self.constraint_entries.len())
            .saturating_add(self.hidden.len());
        let samples = self
            .curves
            .iter()
            .map(|curve| {
                curve
                    .screen_polyline
                    .len()
                    .saturating_add(curve.screen_parameters.len())
            })
            .chain(
                self.computed
                    .iter()
                    .map(|curve| curve.screen_polyline.len()),
            )
            .chain(self.annotations.iter().map(|annotation| {
                annotation
                    .operands
                    .len()
                    .saturating_add(match &annotation.geometry {
                        SceneAnnotationGeometry::Glyph { markers } => markers.len(),
                        _ => 0,
                    })
            }))
            .chain(self.affordances.iter().map(|affordance| {
                affordance
                    .affected_owners
                    .len()
                    .saturating_add(affordance.actions.len())
            }))
            .try_fold(0_usize, usize::checked_add);
        if count > MAX_ITEMS || samples.is_none_or(|samples| samples > MAX_SAMPLES) {
            return Err(DetachedSceneError::ResourceLimit);
        }
        Ok(())
    }
}

fn validate_annotation(
    annotation: &SceneAnnotation,
    document: &SketchDocument,
) -> Result<(), DetachedSceneError> {
    let valid_source = match annotation.item {
        SelectionItem::Dimension(id) => document
            .dimension(id)
            .is_some_and(|dimension| dimension.source_id == annotation.source),
        SelectionItem::Constraint(id) => document
            .constraint(id)
            .is_some_and(|constraint| constraint.source_id == annotation.source),
        _ => false,
    };
    if !valid_source
        || annotation.label_bounds.is_some_and(|bounds| {
            !bounds.min.is_finite()
                || !bounds.max.is_finite()
                || bounds.min.x > bounds.max.x
                || bounds.min.y > bounds.max.y
        })
    {
        return Err(invalid("malformed annotation identity or label bounds"));
    }
    let finite = match &annotation.geometry {
        SceneAnnotationGeometry::Glyph { markers } => markers.iter().all(|marker| {
            marker.anchor.is_finite()
                && marker.leader_from.is_none_or(ScreenPoint::is_finite)
                && marker.rotation_radians.is_finite()
        }),
        SceneAnnotationGeometry::RightAngle {
            vertex,
            first_arm,
            corner,
            second_arm,
        } => [*vertex, *first_arm, *corner, *second_arm]
            .into_iter()
            .all(ScreenPoint::is_finite),
        SceneAnnotationGeometry::LinearDimension {
            measured_first,
            measured_second,
            first,
            second,
            label_anchor,
        } => [
            *measured_first,
            *measured_second,
            *first,
            *second,
            *label_anchor,
        ]
        .into_iter()
        .all(ScreenPoint::is_finite),
        SceneAnnotationGeometry::RadialDimension {
            center,
            edge,
            label_anchor,
            ..
        } => [*center, *edge, *label_anchor]
            .into_iter()
            .all(ScreenPoint::is_finite),
        SceneAnnotationGeometry::AngularDimension {
            vertex,
            first_ray,
            second_ray,
            radius,
            label_anchor,
            ..
        } => {
            radius.is_finite()
                && *radius > 0.0
                && [*vertex, *first_ray, *second_ray, *label_anchor]
                    .into_iter()
                    .all(ScreenPoint::is_finite)
        }
        SceneAnnotationGeometry::Label {
            anchor,
            leader_from,
        } => anchor.is_finite() && leader_from.is_none_or(ScreenPoint::is_finite),
    };
    if !finite {
        return Err(invalid("non-finite annotation geometry"));
    }
    Ok(())
}
