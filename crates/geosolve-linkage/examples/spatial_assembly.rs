// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::{HardValidity, SolverConfig};
use geosolve_linkage::{
    SpatialAssemblyDocumentSession, SpatialAssemblySession, SpatialDocumentId, SpatialExampleKind,
    spatial_example,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = spatial_example(SpatialExampleKind::ShaftBearing, 1.0)?;
    let runtime = SpatialAssemblySession::new(fixture.assembly, SolverConfig::default())?;
    assert_eq!(
        runtime.accepted_result().core_report.hard_validity,
        HardValidity::Valid
    );

    let persistent = SpatialAssemblyDocumentSession::from_accepted_session(
        SpatialDocumentId::from_u128(0x2000),
        &runtime,
    )?;
    let json = persistent.to_json()?;
    let restored = SpatialAssemblyDocumentSession::from_json(&json, SolverConfig::default())?;
    assert_eq!(
        restored.accepted_result().core_report.hard_validity,
        HardValidity::Valid
    );
    assert_eq!(restored.to_json()?, json);

    println!(
        "spatial assembly: rank {}, gauge DOF {}, internal mobility {}",
        restored.accepted_result().core_report.rank,
        restored.session().gauge_report().gauge_dof,
        restored.session().gauge_report().internal_mobility,
    );
    Ok(())
}
