// SPDX-License-Identifier: GPL-3.0-or-later
//! Detached presentation identity transport; no solver or publication authority.
use crate::{
    AnnotationLayoutState, CurvePickContext, DimensionPresentationContext,
    DimensionPresentationState, EditorScene, SelectionItem, SelectionPresentationState,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Authored dimension presentation and disposable automatic layout state.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DetachedDimensionPresentation {
    pub state: DimensionPresentationState,
    pub context: DimensionPresentationContext,
    pub layout: AnnotationLayoutState,
    pub ids: BTreeMap<String, crate::AnnotationLayoutKey>,
}

/// Exact semantic correspondence between independently allocated native presentations.
#[derive(Debug)]
pub struct PresentationMapping {
    source: geosolve_sketch::DocumentId,
    destination: geosolve_sketch::DocumentId,
    bindings: BTreeMap<crate::IntentNativeBinding, crate::IntentNativeBinding>,
}
impl PresentationMapping {
    /// Reverses an unambiguous native binding map.
    ///
    /// # Errors
    /// Rejects absent, ambiguous, foreign or changed native ownership.
    pub fn reverse(&self) -> Result<Self, String> {
        let mut bindings = BTreeMap::new();
        for (source, destination) in &self.bindings {
            if bindings
                .insert(*destination, *source)
                .is_some_and(|previous| previous != *source)
            {
                return Err("Presentation namespace correspondence is not reversible".into());
            }
        }
        Ok(Self {
            source: self.destination,
            destination: self.source,
            bindings,
        })
    }
    /// Builds exact correspondence under stable source symbols.
    ///
    /// # Errors
    /// Rejects absent, ambiguous, foreign or changed native ownership.
    pub fn new(
        source: Option<&crate::ProjectionalPresentationBindings>,
        destination: Option<&crate::ProjectionalPresentationBindings>,
        source_document: geosolve_sketch::DocumentId,
        destination_document: geosolve_sketch::DocumentId,
    ) -> Result<Self, String> {
        let mut bindings = BTreeMap::new();
        if source_document != destination_document {
            let (Some(source), Some(destination)) = (source, destination) else {
                return Err("Prediction namespace correspondence is unavailable".into());
            };
            if source.document != source_document || destination.document != destination_document {
                return Err("Prediction namespace correspondence belongs to another scene".into());
            }
            for (symbol, before) in &source.nodes {
                let after = destination
                    .nodes
                    .get(symbol)
                    .ok_or("Prediction lost a source-owned declaration")?;
                if before.len() != after.len() {
                    return Err("Prediction declaration ownership changed".into());
                }
                for (before, after) in before.iter().zip(after) {
                    if std::mem::discriminant(before) != std::mem::discriminant(after) {
                        return Err("Prediction declaration ownership kind changed".into());
                    }
                    if let Some(previous) = bindings.insert(*before, *after)
                        && previous != *after
                    {
                        return Err("Prediction namespace correspondence is ambiguous".into());
                    }
                }
            }
        }
        Ok(Self {
            source: source_document,
            destination: destination_document,
            bindings,
        })
    }
    /// Personal state may survive a source edit only for unambiguous owners
    /// whose complete binding shape still agrees. Unlike prediction mapping,
    /// absent or changed declarations are simply not reconciled.
    #[must_use]
    pub fn for_replacement(
        source_scene: &EditorScene,
        destination_scene: &EditorScene,
        source_bindings: Option<&crate::ProjectionalPresentationBindings>,
        destination_bindings: Option<&crate::ProjectionalPresentationBindings>,
    ) -> Self {
        use crate::IntentNativeBinding as Binding;
        let source_document = source_scene.presentation_document();
        let destination_document = destination_scene.presentation_document();
        let mut result = Self {
            source: source_document.id(),
            destination: destination_document.id(),
            bindings: BTreeMap::new(),
        };
        if result.source == result.destination {
            return result;
        }
        let (Some(before), Some(after)) = (source_bindings, destination_bindings) else {
            return result;
        };
        let compatible = |before: &Binding, after: &Binding| {
            if std::mem::discriminant(before) != std::mem::discriminant(after) {
                return false;
            }
            match (before, after) {
                (Binding::Curve(before), Binding::Curve(after)) => {
                    match (
                        source_document.curve(*before),
                        destination_document.curve(*after),
                    ) {
                        (Some(before), Some(after)) => {
                            std::mem::discriminant(&before.definition)
                                == std::mem::discriminant(&after.definition)
                        }
                        _ => false,
                    }
                }
                _ => true,
            }
        };
        let mut candidates = BTreeMap::<Binding, std::collections::BTreeSet<Binding>>::new();
        let mut reverse = BTreeMap::<Binding, std::collections::BTreeSet<Binding>>::new();
        for (symbol, before) in &before.nodes {
            let Some(after) = after.nodes.get(symbol) else {
                continue;
            };
            if before.len() != after.len()
                || !before.iter().zip(after).all(|(a, b)| compatible(a, b))
            {
                continue;
            }
            for (before, after) in before.iter().zip(after) {
                candidates.entry(*before).or_default().insert(*after);
                reverse.entry(*after).or_default().insert(*before);
            }
        }
        for (before, after) in candidates {
            if after.len() == 1
                && let Some(&after) = after.first()
                && reverse
                    .get(&after)
                    .is_some_and(|sources| sources.len() == 1)
            {
                result.bindings.insert(before, after);
            }
        }
        result
    }
    fn binding(
        &self,
        source: crate::IntentNativeBinding,
    ) -> Result<crate::IntentNativeBinding, String> {
        if self.source == self.destination {
            return Ok(source);
        }
        self.bindings
            .get(&source)
            .copied()
            .ok_or_else(|| "Prediction has no exact presentation binding".into())
    }
    /// Maps one native selection item.
    ///
    /// # Errors
    /// Rejects absent, ambiguous, foreign or changed native ownership.
    pub fn selection(&self, item: SelectionItem) -> Result<SelectionItem, String> {
        use crate::IntentNativeBinding as Binding;
        Ok(match item {
            SelectionItem::Point(point) => match self.binding(Binding::Point(point))? {
                Binding::Point(point) => SelectionItem::Point(point),
                _ => return Err("Prediction point binding changed kind".into()),
            },
            SelectionItem::Curve(span) => match self.binding(Binding::Curve(span.curve))? {
                Binding::Curve(curve) => SelectionItem::Curve(geosolve_sketch::CurveSpan {
                    curve,
                    segment: span.segment,
                }),
                _ => return Err("Prediction curve binding changed kind".into()),
            },
            SelectionItem::Dimension(dimension) => {
                match self.binding(Binding::Dimension(dimension))? {
                    Binding::Dimension(dimension) => SelectionItem::Dimension(dimension),
                    _ => return Err("Prediction dimension binding changed kind".into()),
                }
            }
            SelectionItem::Constraint(constraint) => {
                match self.binding(Binding::Constraint(constraint))? {
                    Binding::Constraint(constraint) => SelectionItem::Constraint(constraint),
                    _ => return Err("Prediction constraint binding changed kind".into()),
                }
            }
            SelectionItem::Feature(feature) => {
                match self.binding(Binding::ComputedFeature(feature))? {
                    Binding::ComputedFeature(feature) => SelectionItem::Feature(feature),
                    _ => return Err("Prediction feature binding changed kind".into()),
                }
            }
            SelectionItem::FeatureCorner(corner) => match (
                self.binding(Binding::ComputedFeature(corner.feature))?,
                self.binding(Binding::ComputedFeatureCorner(corner.corner))?,
            ) {
                (Binding::ComputedFeature(feature), Binding::ComputedFeatureCorner(corner)) => {
                    SelectionItem::FeatureCorner(geosolve_sketch_features::ComputedCornerRef {
                        feature,
                        corner,
                    })
                }
                _ => return Err("Prediction corner binding changed kind".into()),
            },
            SelectionItem::Datum(datum) => SelectionItem::Datum(datum),
        })
    }
    /// Maps a picked occurrence against its exact destination scene.
    ///
    /// # Errors
    /// Rejects absent, ambiguous, foreign or changed native ownership.
    pub fn curve_pick(
        &self,
        pick: CurvePickContext,
        scene: &EditorScene,
    ) -> Result<CurvePickContext, String> {
        use crate::SceneCurveOrigin;
        let SelectionItem::Curve(span) = self.selection(SelectionItem::Curve(pick.span))? else {
            return Err("Browsing picked curve changed kind".into());
        };
        let origin = match pick.origin {
            SceneCurveOrigin::Native => SceneCurveOrigin::Native,
            SceneCurveOrigin::FilletDiscarded {
                source,
                interval,
                provenance,
                ..
            } => {
                let SelectionItem::Curve(source_span) =
                    self.selection(SelectionItem::Curve(source.span))?
                else {
                    return Err("Browsing implicit curve source changed kind".into());
                };
                let SelectionItem::FeatureCorner(owner) =
                    self.selection(SelectionItem::FeatureCorner(provenance.owner))?
                else {
                    return Err("Browsing implicit curve owner changed kind".into());
                };
                let mut origins = scene.curves.iter().filter_map(|curve| match curve.origin {
                    SceneCurveOrigin::FilletDiscarded {
                        source,
                        interval: native_interval,
                        provenance: native_provenance,
                        ..
                    } if curve.span == span
                        && source.span == source_span
                        && native_interval == interval
                        && native_provenance.owner == owner
                        && native_provenance.endpoint == provenance.endpoint
                        && native_provenance.base_interval == provenance.base_interval =>
                    {
                        Some(curve.origin)
                    }
                    _ => None,
                });
                let origin = origins
                    .next()
                    .ok_or("Browsing picked implicit curve is unavailable")?;
                if origins.next().is_some() {
                    return Err("Browsing picked implicit curve is ambiguous".into());
                }
                origin
            }
        };
        Ok(CurvePickContext {
            span,
            parameter: pick.parameter,
            origin,
        })
    }
    /// Maps one authored annotation layout identity.
    ///
    /// # Errors
    /// Rejects absent, ambiguous, foreign or changed native ownership.
    pub fn layout_key(
        &self,
        key: crate::AnnotationLayoutKey,
    ) -> Result<crate::AnnotationLayoutKey, String> {
        use crate::IntentNativeBinding as Binding;
        if key.document != self.source {
            return Err("Prediction annotation belongs to another document".into());
        }
        let Binding::Source(source) = self.binding(Binding::Source(key.source))? else {
            return Err("Prediction annotation source changed kind".into());
        };
        Ok(crate::AnnotationLayoutKey {
            document: self.destination,
            source,
            item: self.selection(key.item)?,
            ..key
        })
    }
    /// Maps authored dimension intent while discarding automatic layout caches.
    ///
    /// # Errors
    /// Rejects absent, ambiguous, foreign or changed native ownership.
    pub fn dimensions(&self, dimensions: &mut DetachedDimensionPresentation) -> Result<(), String> {
        if self.source == self.destination {
            return Ok(());
        }
        dimensions.ids = dimensions
            .ids
            .iter()
            .map(|(id, key)| Ok((id.clone(), self.layout_key(*key)?)))
            .collect::<Result<_, String>>()?;
        dimensions.layout = AnnotationLayoutState::from_entries(
            dimensions
                .layout
                .entries()
                .into_iter()
                .map(|entry| {
                    Ok(crate::AnnotationLayoutEntry {
                        key: self.layout_key(entry.key)?,
                        placement: entry.placement,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?,
        );
        dimensions.context.generated = dimensions
            .context
            .generated
            .iter()
            .map(|item| self.selection(*item))
            .collect::<Result<_, _>>()?;
        dimensions.context.default_priority = dimensions
            .context
            .default_priority
            .iter()
            .map(|item| self.selection(*item))
            .collect::<Result<_, _>>()?;
        dimensions.context.hovered = dimensions
            .context
            .hovered
            .map(|item| self.selection(item))
            .transpose()?;
        dimensions.context.active = dimensions
            .context
            .active
            .map(|item| self.selection(item))
            .transpose()?;
        // Namespace-specific automatic caches are rebuilt once against the exact
        // prediction scene; authored priorities/manual placements are retained.
        dimensions.state = DimensionPresentationState::default();
        Ok(())
    }
}

/// Translates a validated personal selection between two exact accepted presentations.
///
/// # Errors
/// Rejects stale/foreign occurrences or ambiguous source-owned correspondence.
pub fn map_presentation_selection(
    source: &EditorScene,
    source_bindings: Option<&crate::ProjectionalPresentationBindings>,
    destination: &EditorScene,
    destination_bindings: Option<&crate::ProjectionalPresentationBindings>,
    selection: SelectionPresentationState,
) -> Result<SelectionPresentationState, String> {
    selection.validate(source).map_err(|e| e.to_string())?;
    let mapping = PresentationMapping::new(
        source_bindings,
        destination_bindings,
        source.presentation_document().id(),
        destination.presentation_document().id(),
    )?;
    let mapped = SelectionPresentationState {
        items: selection
            .items
            .into_iter()
            .map(|item| mapping.selection(item))
            .collect::<Result<_, _>>()?,
        curve_picks: selection
            .curve_picks
            .into_iter()
            .map(|pick| mapping.curve_pick(pick, destination))
            .collect::<Result<_, _>>()?,
    };
    mapped.validate(destination).map_err(|e| e.to_string())?;
    Ok(mapped)
}

/// The selection-bearing fields of a detached accepted presentation. Additional
/// paint/layout fields are deliberately outside the editing operand contract.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DetachedSelectionSeed {
    format: String,
    scene_key: String,
    scene: String,
    bindings: Option<crate::ProjectionalPresentationBindings>,
    #[serde(default)]
    visibility_seed: crate::VisibilitySeed,
    #[serde(default)]
    visibility: crate::VisibilityState,
}

/// Personal selection stamped against one exact detached accepted presentation.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DetachedSelectionState {
    format: String,
    scene_key: String,
    selection: Vec<SelectionItem>,
    curve_picks: Vec<CurvePickContext>,
}

/// One tab's accepted-scene selection; it grants no editing capability.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetachedSelectionView {
    seed: DetachedSelectionSeed,
    state: DetachedSelectionState,
}

impl DetachedSelectionView {
    /// Maps only validated accepted-scene selection into the destination namespace.
    ///
    /// # Errors
    /// Rejects stale scenes, invalid native occurrences or ambiguous correspondence.
    pub fn map_to(
        self,
        destination: &EditorScene,
        bindings: Option<&crate::ProjectionalPresentationBindings>,
    ) -> Result<SelectionPresentationState, String> {
        if self.seed.format != "geosolve-local-interaction-v1"
            || self.state.format != self.seed.format
            || self.state.scene_key != self.seed.scene_key
        {
            return Err("Authoring selection belongs to a stale accepted scene".into());
        }
        let mut source =
            EditorScene::from_detached_json(&self.seed.scene).map_err(|e| e.to_string())?;
        self.seed.visibility_seed.validate(&self.seed.visibility)?;
        source
            .hide_items(
                self.seed
                    .visibility_seed
                    .hidden_items(&self.seed.visibility),
            )
            .map_err(|e| e.to_string())?;
        map_presentation_selection(
            &source,
            self.seed.bindings.as_ref(),
            destination,
            bindings,
            SelectionPresentationState {
                items: self.state.selection,
                curve_picks: self.state.curve_picks,
            },
        )
    }
}
