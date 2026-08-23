// SPDX-License-Identifier: GPL-3.0-or-later

use std::hint::black_box;
use std::time::{Duration, Instant};

use geosolve_constraint_editor::{
    ColdIntentMaterializer, IntentNativeBinding, ProjectionalIntentCoordinator,
};
use geosolve_sketch::{DocumentId, OperationControl, PersistentId};
use geosolve_sketch_intent::{
    GeometryRecipeKind, InputRole, InputSlot, IntentFieldKey, IntentKey, IntentLiteral,
    IntentNodeDraft, IntentNodeKind, IntentPatch, IntentPatchOperation, IntentPatchPolicy,
    IntentPortRole, IntentPortSelector, IntentSessionId, IntentUnit, LeafField, PatchPortRef,
};

const POINTS: usize = 96;
const WARMUPS: u32 = 12;
const SAMPLES: u32 = 120;
const PREVIEW_P95_BUDGET: Duration = Duration::from_millis(16);
const TERMINAL_BUDGET: Duration = Duration::from_millis(750);

fn main() {
    let raw = 0x8300_9001_u128;
    let mut coordinator = ProjectionalIntentCoordinator::empty(
        IntentSessionId::from_raw(raw),
        ColdIntentMaterializer::with_default_policy(
            DocumentId(PersistentId::from_u128(raw << 32)),
            1.0,
        )
        .expect("M83 performance materializer"),
    )
    .expect("M83 performance coordinator");
    let fixture = coordinator
        .apply_patch(IntentPatch::new(
            coordinator.intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            fixture_operations(),
        ))
        .expect("representative projectional fixture");
    let point_port = fixture
        .aliases
        .port(&key("p000"), selector(IntentPortRole::Primary))
        .expect("first point output");
    let IntentNativeBinding::Point(point) = coordinator
        .accepted_materialization()
        .expect("fixture authority")
        .ownership
        .port(point_port)
        .expect("first point binding")
    else {
        panic!("first point output must bind a native point")
    };
    let accepted = coordinator
        .accepted_materialization()
        .expect("fixture authority");
    println!(
        "m83/config: declarations={} points={} curves={} warmups={WARMUPS} samples={SAMPLES} statistic=p95-nearest-rank",
        coordinator.intent().graph().nodes().len(),
        accepted.session.design_document().points().len(),
        accepted.session.design_document().curves().len(),
    );

    let history_before = coordinator.intent().undo_len();
    coordinator
        .begin_point_drag(83, point)
        .expect("prepare reverse free-leaf route");
    let mut preview_samples =
        Vec::with_capacity(usize::try_from(SAMPLES).expect("bounded sample count"));
    let final_request = WARMUPS + SAMPLES;
    for request_id in 1..=final_request {
        let target = [
            0.25 + f64::from(request_id) * 0.002,
            0.125 + f64::from(request_id) * 0.001,
        ];
        let started = Instant::now();
        let preview = coordinator
            .preview_point_drag(
                83,
                u64::from(request_id),
                black_box(target),
                OperationControl::unlimited(),
            )
            .expect("retained projectional preview")
            .expect("accepted projectional preview");
        let elapsed = started.elapsed();
        assert!(preview.accepted_position.into_iter().all(f64::is_finite));
        assert_eq!(coordinator.intent().undo_len(), history_before);
        if request_id > WARMUPS {
            preview_samples.push(elapsed);
        }
    }
    report("retained-preview", &preview_samples, PREVIEW_P95_BUDGET);

    let terminal_started = Instant::now();
    let terminal = coordinator
        .finish_point_drag(83, u64::from(final_request))
        .expect("exact cold terminal publication");
    let terminal_elapsed = terminal_started.elapsed();
    black_box(terminal);
    println!(
        "m83/exact-terminal: elapsed={:.3}ms budget={:.3}ms",
        terminal_elapsed.as_secs_f64() * 1_000.0,
        TERMINAL_BUDGET.as_secs_f64() * 1_000.0,
    );
    assert!(
        terminal_elapsed <= TERMINAL_BUDGET,
        "M83 exact terminal publication {terminal_elapsed:?} exceeded {TERMINAL_BUDGET:?}"
    );
    assert_eq!(coordinator.intent().undo_len(), history_before + 1);
    let accepted = coordinator
        .accepted_materialization()
        .expect("terminal accepted authority");
    assert!(accepted.validation.hard_residuals_validated);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
    );
}

fn fixture_operations() -> Vec<IntentPatchOperation> {
    let mut operations = Vec::with_capacity(POINTS * 2 - 1);
    for index in 0..POINTS {
        let alias = point_key(index);
        let index = u32::try_from(index).expect("bounded representative point index");
        let position = [f64::from(index) * 0.2, f64::from(index % 7) * 0.15];
        let draft = IntentNodeDraft::new(
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::SketchPoint,
            },
            alias.clone(),
        )
        .with_instance_leaf(
            selector(IntentPortRole::Primary),
            LeafField::X,
            coordinate(position[0]),
        )
        .with_instance_leaf(
            selector(IntentPortRole::Primary),
            LeafField::Y,
            coordinate(position[1]),
        );
        operations.push(IntentPatchOperation::CreateNode {
            alias,
            draft: Box::new(draft),
            cell: None,
        });
    }
    for index in 0..POINTS - 1 {
        let alias = key(&format!("s{index:03}"));
        let draft = IntentNodeDraft::new(
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::Segment,
            },
            alias.clone(),
        )
        .with_input(
            InputSlot::new(InputRole::Point, 0),
            PatchPortRef::Alias {
                node: point_key(index),
                selector: selector(IntentPortRole::Primary),
            },
        )
        .with_input(
            InputSlot::new(InputRole::Point, 1),
            PatchPortRef::Alias {
                node: point_key(index + 1),
                selector: selector(IntentPortRole::Primary),
            },
        )
        .with_field(
            IntentFieldKey(key("branch_direction")),
            IntentLiteral::Point([1.0, 0.0]),
        );
        operations.push(IntentPatchOperation::CreateNode {
            alias,
            draft: Box::new(draft),
            cell: None,
        });
    }
    operations
}

fn report(name: &str, samples: &[Duration], budget: Duration) {
    let mut ordered = samples.to_vec();
    ordered.sort_unstable();
    let median = ordered[ordered.len() / 2];
    let p95_index = (ordered.len() * 95).div_ceil(100).saturating_sub(1);
    let p95 = ordered[p95_index];
    println!(
        "m83/{name}: median={:.3}ms p95={:.3}ms budget={:.3}ms",
        median.as_secs_f64() * 1_000.0,
        p95.as_secs_f64() * 1_000.0,
        budget.as_secs_f64() * 1_000.0,
    );
    assert!(p95 <= budget, "M83 {name} p95 {p95:?} exceeded {budget:?}");
}

fn point_key(index: usize) -> IntentKey {
    key(&format!("p{index:03}"))
}

fn key(value: &str) -> IntentKey {
    IntentKey::new(value).expect("bounded static M83 performance key")
}

const fn selector(role: IntentPortRole) -> IntentPortSelector {
    IntentPortSelector::Node { role, index: 0 }
}

const fn coordinate(value: f64) -> IntentLiteral {
    IntentLiteral::Quantity {
        value,
        unit: IntentUnit::Length,
    }
}
