// SPDX-License-Identifier: GPL-3.0-or-later
//! Trusted historical replay witnesses. These do not authenticate collaboration lifetimes.
use std::collections::{BTreeMap, BTreeSet};

use geosolve_sketch_code::{ManagedDeclarationDraft, ManagedValue, SemanticSymbol};
use serde::{Deserialize, Serialize};

use crate::{EditableSession, EngineError, PointGestureTarget};

fn error(message: &str) -> EngineError {
    EngineError::Admission(message.into())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConstructionAllocation {
    pub provisional: String,
    pub persistent: String,
    pub provisional_variable: String,
    pub persistent_variable: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConstructionReplayWitness {
    pub required_stable_declarations: Vec<SemanticSymbol>,
    pub allocation_mapping: Vec<ConstructionAllocation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PointReplayWitness {
    pub required_stable_declarations: Vec<SemanticSymbol>,
    pub resolved_target: PointGestureTarget,
}

pub(super) fn same_project(
    latest: &EditableSession,
    basis: &EditableSession,
) -> Result<(), EngineError> {
    if latest.code_snapshot().project != basis.code_snapshot().project {
        return Err(error("gesture replay basis belongs to a different project"));
    }
    Ok(())
}

pub(super) fn references(value: &ManagedValue, found: &mut BTreeSet<SemanticSymbol>) {
    match value {
        ManagedValue::Reference { declaration, .. } => {
            found.insert(declaration.clone());
        }
        ManagedValue::Array(values) => values.iter().for_each(|value| references(value, found)),
        ManagedValue::Object(values) => values.values().for_each(|value| references(value, found)),
        _ => {}
    }
}

fn rename_references(value: &mut ManagedValue, mapping: &BTreeMap<String, String>) {
    match value {
        ManagedValue::Reference { declaration, .. } => {
            if let Some(persistent) = mapping.get(&declaration.0) {
                declaration.0.clone_from(persistent);
            }
        }
        ManagedValue::Array(values) => values
            .iter_mut()
            .for_each(|value| rename_references(value, mapping)),
        ManagedValue::Object(values) => values
            .values_mut()
            .for_each(|value| rename_references(value, mapping)),
        _ => {}
    }
}

pub(super) fn construction_witness(
    original: &[ManagedDeclarationDraft],
    latest: &[ManagedDeclarationDraft],
) -> Result<ConstructionReplayWitness, EngineError> {
    if original.len() != latest.len() {
        return Err(error("latest construction changed declaration count"));
    }
    let allocation_mapping = original
        .iter()
        .zip(latest)
        .map(|(old, new)| ConstructionAllocation {
            provisional: old.symbol.clone(),
            persistent: new.symbol.clone(),
            provisional_variable: old.variable.clone(),
            persistent_variable: new.variable.clone(),
        })
        .collect::<Vec<_>>();
    let mapping = allocation_mapping
        .iter()
        .map(|entry| (entry.provisional.clone(), entry.persistent.clone()))
        .collect::<BTreeMap<_, _>>();
    let unique =
        |values: Vec<&str>| values.iter().copied().collect::<BTreeSet<_>>().len() == values.len();
    if mapping.len() != original.len()
        || !unique(latest.iter().map(|value| value.symbol.as_str()).collect())
        || !unique(
            original
                .iter()
                .map(|value| value.variable.as_str())
                .collect(),
        )
        || !unique(latest.iter().map(|value| value.variable.as_str()).collect())
    {
        return Err(error("construction allocation is not bijective"));
    }
    let mut required = BTreeSet::new();
    for (old, new) in original.iter().zip(latest) {
        references(&old.arguments, &mut required);
        let mut normalized = old.clone();
        normalized.symbol.clone_from(&new.symbol);
        normalized.variable.clone_from(&new.variable);
        rename_references(&mut normalized.arguments, &mapping);
        if normalized != *new {
            return Err(error(
                "latest construction changed semantic operands, shape or branch intent",
            ));
        }
    }
    required.retain(|symbol| !mapping.contains_key(&symbol.0));
    // A new local name may not capture an originally external reference.
    if required
        .iter()
        .any(|symbol| mapping.values().any(|name| name == &symbol.0))
    {
        return Err(error(
            "construction allocation captured an external reference",
        ));
    }
    Ok(ConstructionReplayWitness {
        required_stable_declarations: required.into_iter().collect(),
        allocation_mapping,
    })
}

fn owner_declaration(address: &geosolve_sketch_code::CodeWritableAddress) -> SemanticSymbol {
    match &address.owner.address {
        geosolve_sketch_code::CodeOwnerAddress::DirectDeclaration { declaration } => {
            declaration.clone()
        }
        geosolve_sketch_code::CodeOwnerAddress::GeneratedMember { address } => {
            SemanticSymbol(address.invocation.clone())
        }
    }
}
fn target_addresses(
    target: &PointGestureTarget,
) -> Vec<&geosolve_sketch_code::CodeWritableAddress> {
    match target {
        PointGestureTarget::Point { address } => vec![address],
        PointGestureTarget::RectangleCorner {
            lower_left,
            upper_right,
            ..
        } => vec![lower_left, upper_right],
    }
}

// Only accepted family-owned writable coordinate paths and dimensional unit values
// are continuous here. Bare numeric branch/index arguments remain exact.
fn erase_point(value: &mut ManagedValue, path: &[geosolve_sketch_code::ManagedPathSegment]) {
    use geosolve_sketch_code::ManagedPathSegment;
    let Some((head, tail)) = path.split_first() else {
        if matches!(value, ManagedValue::Array(values) if values.len() == 2 && values.iter().all(|v| matches!(v, ManagedValue::Number(_) | ManagedValue::Unit(_))))
        {
            *value = ManagedValue::Null;
        }
        return;
    };
    let next = match (value, head) {
        (ManagedValue::Object(fields), ManagedPathSegment::Field(field)) => fields.get_mut(field),
        (ManagedValue::Array(values), ManagedPathSegment::Index(index)) => values.get_mut(*index),
        (ManagedValue::Array(values), ManagedPathSegment::Member { member }) => values.iter_mut().find(|value| matches!(value, ManagedValue::Object(fields) if fields.get("key") == Some(&ManagedValue::String(member.clone())))),
        _ => None,
    };
    if let Some(next) = next {
        erase_point(next, tail);
    }
}
fn erase_unit_values(value: &mut ManagedValue) {
    match value {
        ManagedValue::Unit(unit) => unit.value = 0.0,
        ManagedValue::Object(fields) => fields.values_mut().for_each(erase_unit_values),
        ManagedValue::Array(values) => values.iter_mut().for_each(erase_unit_values),
        _ => {}
    }
}
fn source_codec(
    session: &EditableSession,
    declaration: &geosolve_sketch_code::AuthoringDeclaration,
) -> ManagedValue {
    let mut arguments = declaration.arguments.clone();
    for lens in &session.accepted().0.materialized.expansion.writable_points {
        let addresses = match &lens.edit {
            geosolve_sketch_code::CodePointEdit::Point { address } => vec![address],
            geosolve_sketch_code::CodePointEdit::RectangleCorner {
                lower_left,
                upper_right,
                ..
            } => vec![lower_left, upper_right],
        };
        for address in addresses {
            if matches!(&address.owner.address, geosolve_sketch_code::CodeOwnerAddress::DirectDeclaration { declaration: symbol } if symbol == &declaration.symbol)
            {
                erase_point(&mut arguments, &address.output.0);
            }
        }
    }
    erase_unit_values(&mut arguments);
    arguments
}

fn point_dependencies(
    program: &geosolve_sketch_code::AuthoringProgram,
    target: &PointGestureTarget,
) -> BTreeSet<SemanticSymbol> {
    // Follow both operands and consumers so constraints/features affecting this point
    // retain their explicit discrete choices. Unconnected edits do not enter the guard.
    let mut adjacency = BTreeMap::<SemanticSymbol, BTreeSet<SemanticSymbol>>::new();
    for (symbol, value, patch) in program
        .declarations
        .iter()
        .map(|declaration| {
            (
                &declaration.symbol,
                &declaration.arguments,
                declaration.patch.as_ref(),
            )
        })
        .chain(
            program
                .scalar_bindings
                .iter()
                .map(|binding| (&binding.symbol, &binding.value, None)),
        )
    {
        let mut refs = BTreeSet::new();
        references(value, &mut refs);
        if let Some(patch) = patch {
            references(&patch.arguments, &mut refs);
        }
        for reference in refs {
            adjacency
                .entry(symbol.clone())
                .or_default()
                .insert(reference.clone());
            adjacency
                .entry(reference)
                .or_default()
                .insert(symbol.clone());
        }
    }
    let mut pending = target_addresses(target)
        .into_iter()
        .map(|a| owner_declaration(a).clone())
        .collect::<Vec<_>>();
    let mut required = BTreeSet::new();
    while let Some(symbol) = pending.pop() {
        if required.insert(symbol.clone())
            && let Some(neighbors) = adjacency.get(&symbol)
        {
            pending.extend(neighbors.iter().cloned());
        }
    }
    required
}

pub(super) fn point_source_guard(
    latest: &EditableSession,
    basis: &EditableSession,
    target: &PointGestureTarget,
) -> Result<Vec<SemanticSymbol>, EngineError> {
    let old_snapshot = basis.code_snapshot();
    let new_snapshot = latest.code_snapshot();
    let old = old_snapshot
        .code_project
        .as_ref()
        .ok_or_else(|| error("missing replay basis project"))?;
    let new = new_snapshot
        .code_project
        .as_ref()
        .ok_or_else(|| error("missing latest project"))?;
    let originals = old
        .managed
        .program
        .declarations
        .iter()
        .map(|d| (d.symbol.clone(), d))
        .collect::<BTreeMap<_, _>>();
    let currents = new
        .managed
        .program
        .declarations
        .iter()
        .map(|d| (d.symbol.clone(), d))
        .collect::<BTreeMap<_, _>>();
    let original_bindings = old
        .managed
        .program
        .scalar_bindings
        .iter()
        .map(|binding| (binding.symbol.clone(), binding))
        .collect::<BTreeMap<_, _>>();
    let current_bindings = new
        .managed
        .program
        .scalar_bindings
        .iter()
        .map(|binding| (binding.symbol.clone(), binding))
        .collect::<BTreeMap<_, _>>();
    let required = point_dependencies(&old.managed.program, target);
    for symbol in &required {
        if let Some(original) = original_bindings.get(symbol) {
            let current = current_bindings
                .get(symbol)
                .ok_or_else(|| error("point replay scalar source owner removed or replaced"))?;
            let mut previous = original.value.clone();
            let mut next = current.value.clone();
            erase_unit_values(&mut previous);
            erase_unit_values(&mut next);
            if previous != next {
                return Err(error(
                    "point replay scalar codec, operands or units changed",
                ));
            }
            continue;
        }
        let original = originals
            .get(symbol)
            .ok_or_else(|| error("point replay source owner missing at basis"))?;
        let current = currents
            .get(symbol)
            .ok_or_else(|| error("point replay source owner removed"))?;
        if original.builder_path != current.builder_path
            || original.patch != current.patch
            || source_codec(basis, original) != source_codec(latest, current)
        {
            return Err(error(
                "point replay source codec, operands or explicit branch changed",
            ));
        }
        if original.patch.is_some()
            && (old.custom_files != new.custom_files
                || old.artifacts != new.artifacts
                || old.lock != new.lock
                || old_snapshot.generated != new_snapshot.generated)
        {
            return Err(error("point replay custom patch execution context changed"));
        }
    }
    if target_addresses(target).iter().any(|address| {
        matches!(
            address.owner.address,
            geosolve_sketch_code::CodeOwnerAddress::GeneratedMember { .. }
        )
    }) && old_snapshot.generated != new_snapshot.generated
    {
        return Err(error("point replay generated codec changed"));
    }
    Ok(required.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(symbol: &str, reference: &str) -> ManagedDeclarationDraft {
        ManagedDeclarationDraft {
            symbol: symbol.into(),
            variable: symbol.into(),
            builder_path: vec!["constraint".into(), "horizontal".into()],
            arguments: ManagedValue::Object(BTreeMap::from([
                (
                    "operand".into(),
                    ManagedValue::Reference {
                        declaration: SemanticSymbol(reference.into()),
                        path: geosolve_sketch_code::SemanticOutputPath::default(),
                    },
                ),
                (
                    "branchDirection".into(),
                    ManagedValue::Array(vec![ManagedValue::Number(1.0), ManagedValue::Number(0.0)]),
                ),
                ("label".into(), ManagedValue::String("old".into())),
            ])),
            patch: None,
            group: None,
            suppressed: None,
            comments: None,
        }
    }
    #[test]
    fn alpha_replay_renames_only_typed_local_references_and_rejects_all_meaning_drift() {
        let original = vec![draft("old", "external"), draft("child", "old")];
        let latest = vec![draft("new", "external"), draft("next", "new")];
        let witness = construction_witness(&original, &latest).unwrap();
        assert_eq!(
            witness.required_stable_declarations,
            vec![SemanticSymbol("external".into())]
        );
        for mutation in 0..7 {
            let mut changed = latest.clone();
            match mutation {
                0 => changed[0].arguments = draft("new", "other").arguments,
                1 => changed[0].builder_path = vec!["geometry".into(), "segment".into()],
                2 => changed[0].patch = Some("foreign".into()),
                3 => changed[0].suppressed = Some(true),
                4 => changed[0].comments = Some(vec!["changed".into()]),
                5 | 6 => {
                    let ManagedValue::Object(fields) = &mut changed[0].arguments else {
                        panic!()
                    };
                    if mutation == 5 {
                        fields.insert(
                            "branchDirection".into(),
                            ManagedValue::Array(vec![
                                ManagedValue::Number(-1.0),
                                ManagedValue::Number(0.0),
                            ]),
                        );
                    } else {
                        fields.insert("label".into(), ManagedValue::String("new".into()));
                    }
                }
                _ => unreachable!(),
            }
            assert!(
                construction_witness(&original, &changed).is_err(),
                "mutation {mutation}"
            );
        }
    }
}
