<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# GeoSolve documentation

GeoSolve is an embeddable Rust constraint solver with a headless UI adapter and
bidirectional TypeScript authoring. The [project README](../README.md) introduces
its purpose and links the [browser demo](https://arduano.github.io/geometric-constraint-solver/).

## Choose a starting point

| I want to… | Read |
| --- | --- |
| Build the demo or run the local server | [Getting started](GETTING_STARTED.md) |
| Author sketches in code and the UI | [Authoring guide](AUTHORING.md) |
| Use the CLI, agent commands or shared server | [CLI guide](../packages/geosolve-cli/README.md) |
| Embed a solver or editor | [Architecture](../ARCHITECTURE.md) and [API compatibility](API_COMPATIBILITY.md) |
| Use Rust sketch APIs | [Sketch crate](../crates/geosolve-sketch) |
| Embed headless interaction | [Constraint editor](../crates/geosolve-constraint-editor) and [engine](../crates/geosolve-sketch-engine/README.md) |
| Embed TypeScript/WASM | [Engine package](../packages/geosolve-engine/README.md) and [authoring SDK](../packages/geosolve-sketch-code/README.md) |
| Understand shared editing | [Collaboration package](../packages/geosolve-collaboration/README.md) and [protocol](M98_COLLABORATION.md) |
| Export or render without a browser | [Headless CLI](../crates/geosolve-headless) |
| Contribute or investigate a failure | [Development](DEVELOPMENT.md), [agent instructions](../AGENTS.md) and [scenarios](SCENARIOS.md) |
| Qualify a release | [Release qualification](RELEASE_QUALIFICATION.md) and [acceptance](../ACCEPTANCE.md) |
| Understand current work | [Roadmap](../PLAN.md) and [M100](M100_FINAL_CLEANUP.md) |

## Examples

Start with the small [editable folder project](../examples/file-workspace/README.md).
The [water manifold](../examples/file-workspace-manifold/README.md) demonstrates
real rounded channels, reusable patches and source-defined overview dimensions.
The [generator website](../examples/generator-website/README.md) demonstrates an
application using generated results. [Baked profiles](../examples/file-workspace-bake/README.md)
show planar export for a downstream consumer.

## Technical references and history

The [detailed architecture](reference/ARCHITECTURE_DETAILS.md) and
[detailed acceptance](reference/ACCEPTANCE_DETAILS.md) preserve mathematical,
ownership and historical contracts. The [ADR index](adr/README.md) organizes design
decisions. [References](../REFERENCES.md) collects mathematical and implementation
sources; [third-party notices](../THIRD_PARTY_LICENSES.md) records attribution.

The [milestone index](history/README.md), [historical roadmap](history/ROADMAP.md)
and [changelog](../CHANGELOG.md) preserve development history. Milestone documents
record evidence at their original checkpoints. Use the current guides for startup
commands and status; historical artifacts, snapshots and endpoints may no longer
be available.
