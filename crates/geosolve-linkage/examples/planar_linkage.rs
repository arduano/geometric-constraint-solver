// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::SolverConfig;
use geosolve_linkage::{
    LinkageSource, PlanarDocumentId, PlanarLinkageDocument, PlanarLinkageSession, four_bar_open,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (linkage, ids) = four_bar_open()?;
    let (document, runtime_map) =
        PlanarLinkageDocument::from_linkage(PlanarDocumentId::from_u128(0x1000), &linkage)?;
    let driver = runtime_map
        .persistent_source(LinkageSource::Driver(ids.driver))
        .expect("captured driver has a persistent source ID");
    let session = PlanarLinkageSession::new(document, SolverConfig::default())?;
    assert!(session.accepted_result().accepted());

    let velocity = session.velocity(driver, 1.0)?;
    assert!(velocity.differentiated_residual_max <= 1.0e-9);

    let json = session.document().to_json()?;
    let restored = PlanarLinkageDocument::from_json(&json)?;
    let restored = PlanarLinkageSession::new(restored, SolverConfig::default())?;
    assert!(restored.accepted_result().accepted());
    assert_eq!(restored.document().to_json()?, json);

    println!(
        "planar linkage: rank {}, internal mobility {}, velocity residual {:.3e}",
        restored.accepted_result().core_report.rank,
        restored.gauge_report().internal_mobility,
        velocity.differentiated_residual_max,
    );
    Ok(())
}
