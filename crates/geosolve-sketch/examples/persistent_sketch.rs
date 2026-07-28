// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::SolverConfig;
use geosolve_sketch::{
    AlphaScenarioIds, AlphaScenarioKind, DocumentCommand, DocumentEdit, SketchDocument,
    SketchDocumentSession, alpha_scenario,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = alpha_scenario(AlphaScenarioKind::A1, 1.0)?;
    let AlphaScenarioIds::A1(ids) = fixture.ids else {
        unreachable!("A1 builder returned different scenario IDs");
    };
    let mut session =
        SketchDocumentSession::new(fixture.document, fixture.request, SolverConfig::default())?;
    assert!(session.accepted_result().accepted());

    let edited = session.apply(DocumentCommand::new(
        session.revision(),
        DocumentEdit::SetScalarValue {
            scalar: ids.rectangle.targets[0],
            value: 6.0,
        },
    ))?;
    assert!(edited.accepted());

    let json = session.document().to_canonical_json()?;
    let restored = SketchDocument::from_json(&json)?;
    let restored = SketchDocumentSession::new(restored, fixture.request, SolverConfig::default())?;
    assert!(restored.accepted_result().accepted());
    assert_eq!(restored.document().to_canonical_json()?, json);

    let report = restored.accepted_result();
    println!(
        "sketch v{}: rank {}, local DOF {}, max normalized residual {:.3e}",
        restored.document().version(),
        report.accepted_view().unstable_core_report().rank,
        report
            .accepted_view()
            .unstable_core_report()
            .local_degrees_of_freedom,
        report
            .accepted_view()
            .unstable_core_report()
            .hard_residual_max,
    );
    Ok(())
}
