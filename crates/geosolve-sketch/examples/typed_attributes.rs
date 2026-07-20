// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch::{AlphaScenarioIds, AlphaScenarioKind, SketchAttributes, alpha_scenario};

#[derive(Debug)]
enum CadAttribute {
    Layer(&'static str),
    ExternalRule { system: &'static str, key: u64 },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = alpha_scenario(AlphaScenarioKind::A1, 1.0)?;
    let AlphaScenarioIds::A1(ids) = fixture.ids else {
        unreachable!()
    };
    let mut attributes = SketchAttributes::new(&fixture.document);
    attributes.insert(
        &fixture.document,
        ids.rectangle.curves[0],
        CadAttribute::Layer("profile"),
    )?;
    let source = fixture
        .document
        .dimension(ids.diagonal)
        .expect("scenario dimension exists")
        .source_id;
    attributes.insert(
        &fixture.document,
        source,
        CadAttribute::ExternalRule {
            system: "host-pdm",
            key: 42,
        },
    )?;

    for (target, attribute) in attributes.iter() {
        match attribute {
            CadAttribute::Layer(layer) => println!("{} {target:?}: layer {layer}", target.kind()),
            CadAttribute::ExternalRule { system, key } => {
                println!("{} {target:?}: {system} rule {key}", target.kind());
            }
        }
    }
    assert!(!fixture.document.to_canonical_json()?.contains("host-pdm"));
    Ok(())
}
