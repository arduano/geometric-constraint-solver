# Coordinated narrow GeoSolve -> MiniCAD bridge, v1
Owner-approved 2026-09-08 22:40 AEST. Two agents, separate write ownership. No full case modelling, M97 integration, new DSL or hardening campaign.

Interchange contract agreed by coordinator for both implementations:
JSON geometry data only, not modeling program. Required fields:
{
 "format":"geosolve-baked-profile-v1",
 "units":"mm",
 "source":{"sha256":"<64 lowercase hex of exact UTF-8 sketch.ts bytes>"},
 "plane":{"origin":[0,0,0],"x_axis":[1,0,0],"y_axis":[0,1,0]},
 "sampling":{"max_chord_error_mm":0.02},
 "regions":[{"id":"region-0","outer":[[0,0],[85,0],[85,56],[0,56]],"holes":[]}]
}
Extra optional provenance fields okay; required meanings fixed. Plane coordinates in mm; orthonormal/right-handed frame. Local XY coordinates in mm, loops implicitly closed (do not repeat first point). OuterCCW/holesCW. Region IDs are file-local identifiers, NOT persistent topology naming; importer requires explicit region ID if more than one. No nonfinite coordinates, invalid/empty/incomplete output, unsupported format/units. sampling.max_chord_error_mm is a bound for source-curve polygonization, not total Boolean/STL manufacturing error; exporter must not claim a bound it cannot support. Initial lines/circles/arcs sufficient; others fail explicitly until implemented. Straight edges meet any positive sampling target. Source hash binds to current accepted disk content; exporter must fail on invalid current source, not silently export recovery/last-good geometry.

Exporter CLI intended spelling:
node scripts/file-workspace.mjs bake <folder> --out <file> --chord-error-mm 0.02
May add a region option, but default export all valid production regions is fine; consumer selects. Headless from disk, no running UI/token/browser required. Output only on successful accepted topology/sample validation. Error nonzero with useful diagnostics. No rendering-polygon/viewport or source-seed export.

GeoSolve agent owns exporter and generic fixture projects ONLY under M98 worktree. Provide working examples/file-workspace-bake/pi-footprint (85x56 rectangle with four2.7mmholes at(3.5,3.5),(61.5,3.5),(3.5,52.5),(61.5,52.5)) and a simple circle/arc test. This Pi outline is explicitly newly authorized bounded sample, not enclosure design. Record real run outputs and invocation in docs/M98_BAKE_HANDOFF.md. Prefer durable source fixtures tracked, output under own target/m98/bake. Don't modify minicad or primaryGeoSolve. Preserve M98 browser/file-sync regressions. No direct cross-agent messages; coordinator handles reconciliation. If contract fails concrete source/API constraints, report exact minimal amendment rather than silently drift.

MiniCAD agent owns reader/examples ONLY under minicad. Implement serde/data reader -> BakedSketch, retained ordinaryRust SDK. Can read M98 worktree contract/source and completed exporter artifacts, but do not invoke/rebuild evolving exporter until coordinator announces readiness. Build importer against contract fixtures now; don't claim fixture is real GeoSolve output. Expected final integration invokes real exporter into minicad-owned output then loads real result; no stub in final demo. Use read-only completed upstream plus private input folder/output to avoid editing M98 demo sources. Keep CLI/model trusted local Rust.

Bounded sample: Pi PCB nominal85x56x1.6mm (thickness assumption) with nominal hole pattern; optional named coarse blocks for USB/Ethernet/cooler/HAT85x56x1.6/SSD22x80 chosen placement and component heights clearly labelled approximations. Simplest sufficient assembly uses exportedPi footprint extruded1.6, plus named ordinaryRust cuboids illustrating occupied envelope/stack. Board gap20mm and any component sizes/positions are provisional, not hardwarefit proof. Current user confirms Pi5 stockcooler/ElecrowtopHAT holes match, PETGlikely; exact componentenvelopes unverified. No standoffs/lid/retention case design. Separate STLs for reference bodies or deliberate union for combined solid; avoid overlapping disconnected shells masquerading as one verified part. Provide lightweight visual artifact with labels and approximate dimension list, not a new UI.

Final proof: actual source+acceptedGeoSolvebake ->MiniCADread+extrude+position ->STL ->independentreload/topology/bounds/volume; change one source dimension in private fixture, rebake/rerun and verify changedSTL. Retain script/source/commands and sourcehash/inputrevision evidence, focused tests. No general framework, full release gates, agent collisions or unrelated source cleanup.
