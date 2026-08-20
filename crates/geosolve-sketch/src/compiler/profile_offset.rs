// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::{
    AuditBinding, Problem, ResidualBlock, ResidualCategory, ResidualId, SourceConstraint,
    SourceConstraintId,
};

use crate::model::{
    ArcId, DimensionKind, DimensionMode, Sketch, SketchCurve, SketchDimensionId, SketchError,
};
use crate::residuals::{
    CurveParameterIncidence, GenericCurveIncidence, ProfileOffsetLineResidual,
    ProfileOffsetRadialResidual, ProfileOffsetTangentialAnchorResidual, ScalarTargetResidual,
};

use super::{
    ArcAngleRole, ArcAngleVariableMapping, ArcRadiusVariableMapping, CircleRadiusVariableMapping,
    IncidenceBuilder, PointVariableMapping, SketchSource, SketchSourceMapping, arc_angle_variable,
    arc_radius_variable, audit_row, audit_row_unit, circle_radius_variable,
    generic_curve_incidence, point_variable, segment_incidence,
};

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn compile_profile_offset_dimension(
    sketch: &Sketch,
    problem: &mut Problem,
    point_variables: &[PointVariableMapping],
    circle_radius_variables: &[CircleRadiusVariableMapping],
    arc_radius_variables: &[ArcRadiusVariableMapping],
    arc_angle_variables: &[ArcAngleVariableMapping],
    dimension_id: SketchDimensionId,
    dimension: &crate::SketchDimension,
    kind: DimensionKind,
) -> Result<SketchSourceMapping, SketchError> {
    let DimensionKind::ProfileOffset { profile, target } = kind else {
        unreachable!("non-profile-offset dimension reached profile-offset compiler");
    };
    if dimension.mode() != DimensionMode::Driving {
        return Err(SketchError::ProfileOffsetRequiresDriving);
    }
    let association = sketch
        .profile_offsets
        .get(profile)
        .ok_or(SketchError::UnknownProfileOffset(profile))?;
    let label = format!(
        "dimension {}: grouped profile offset = {target} (driving)",
        dimension.ordinal()
    );
    let source_id = problem.add_source(SourceConstraint::new(&label)?);
    let mut residual_ids = Vec::new();
    match &association.operand {
        crate::ProfileOffsetOperand::Face {
            direction,
            outer,
            holes,
        } => {
            compile_profile_offset_path(
                sketch,
                problem,
                point_variables,
                circle_radius_variables,
                arc_radius_variables,
                arc_angle_variables,
                source_id,
                "outer",
                &outer.edges,
                &outer.junctions,
                true,
                direction.left_normal_sign(),
                target,
                &mut residual_ids,
            )?;
            for (hole_index, hole) in holes.iter().enumerate() {
                let selected_path = format!("hole {hole_index}");
                compile_profile_offset_path(
                    sketch,
                    problem,
                    point_variables,
                    circle_radius_variables,
                    arc_radius_variables,
                    arc_angle_variables,
                    source_id,
                    &selected_path,
                    &hole.edges,
                    &hole.junctions,
                    true,
                    direction.left_normal_sign(),
                    target,
                    &mut residual_ids,
                )?;
            }
        }
        crate::ProfileOffsetOperand::OpenChain { side, chain } => {
            compile_profile_offset_path(
                sketch,
                problem,
                point_variables,
                circle_radius_variables,
                arc_radius_variables,
                arc_angle_variables,
                source_id,
                "open chain",
                &chain.edges,
                &chain.junctions,
                false,
                side.sign(),
                target,
                &mut residual_ids,
            )?;
        }
    }
    Ok(SketchSourceMapping {
        source: SketchSource::Dimension(dimension_id),
        source_label: label,
        core_source_id: Some(source_id),
        residual_ids,
    })
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn compile_profile_offset_path(
    sketch: &Sketch,
    problem: &mut Problem,
    point_variables: &[PointVariableMapping],
    circle_radius_variables: &[CircleRadiusVariableMapping],
    arc_radius_variables: &[ArcRadiusVariableMapping],
    arc_angle_variables: &[ArcAngleVariableMapping],
    source_id: SourceConstraintId,
    selected_path: &str,
    edges: &[crate::ProfileOffsetEdgePair],
    junctions: &[crate::ProfileOffsetJunctionBranch],
    closed: bool,
    left_normal_sign: f64,
    target: f64,
    residual_ids: &mut Vec<ResidualId>,
) -> Result<(), SketchError> {
    for (edge_index, edge) in edges.iter().enumerate() {
        let source_label = profile_offset_curve_label(sketch, edge.source.curve)?;
        let target_label = profile_offset_curve_label(sketch, edge.target.curve)?;
        let bindings = vec![
            AuditBinding::new("selected path", selected_path),
            AuditBinding::new("edge", edge_index.to_string()),
            AuditBinding::new("source", source_label),
            AuditBinding::new("target", target_label),
            AuditBinding::new("source traversal", format!("{:?}", edge.source.traversal)),
            AuditBinding::new("target traversal", format!("{:?}", edge.target.traversal)),
            AuditBinding::new("distance", target.to_string()),
        ];
        match (edge.source.curve, edge.target.curve) {
            (
                crate::ProfileOffsetCurve::Line(source),
                crate::ProfileOffsetCurve::Line(target_line),
            ) => {
                let mut incidence = IncidenceBuilder::default();
                let source_points =
                    segment_incidence(sketch, point_variables, &mut incidence, source)?;
                let target_points =
                    segment_incidence(sketch, point_variables, &mut incidence, target_line)?;
                residual_ids.push(problem.add_residual(ResidualBlock::new(
                    source_id,
                    ResidualCategory::Hard,
                    incidence.variables,
                    2,
                    vec![1.0, sketch.model_scale],
                    vec![
                        audit_row(
                            "cross(unit(directed_source), unit(directed_target))".into(),
                            bindings.clone(),
                        ),
                        audit_row(
                            "(dot(directed_target.start - directed_source.start, left_normal(unit(directed_source))) - signed_distance) / model_scale".into(),
                            bindings,
                        ),
                    ],
                    ProfileOffsetLineResidual {
                        source: source_points,
                        target: target_points,
                        source_traversal: edge.source.traversal,
                        target_traversal: edge.target.traversal,
                        signed_distance: left_normal_sign * target,
                    },
                )?)?);
            }
            (
                crate::ProfileOffsetCurve::CircularArc(source),
                crate::ProfileOffsetCurve::CircularArc(target_arc),
            ) => {
                let turn = profile_offset_curve_turn(sketch, edge.source)?;
                compile_profile_offset_radial_block(
                    sketch,
                    problem,
                    point_variables,
                    circle_radius_variables,
                    arc_radius_variables,
                    source_id,
                    edge.source.curve,
                    edge.target.curve,
                    -left_normal_sign * turn * target,
                    bindings.clone(),
                    residual_ids,
                )?;
                compile_profile_offset_source_angle_preferences(
                    sketch,
                    problem,
                    arc_angle_variables,
                    source_id,
                    source,
                    &bindings,
                    residual_ids,
                )?;
                let _ = (source, target_arc);
            }
            (crate::ProfileOffsetCurve::Circle(_), crate::ProfileOffsetCurve::Circle(_)) => {
                let turn = profile_offset_curve_turn(sketch, edge.source)?;
                compile_profile_offset_radial_block(
                    sketch,
                    problem,
                    point_variables,
                    circle_radius_variables,
                    arc_radius_variables,
                    source_id,
                    edge.source.curve,
                    edge.target.curve,
                    -left_normal_sign * turn * target,
                    bindings,
                    residual_ids,
                )?;
            }
            _ => {
                return Err(SketchError::InvalidProfileOffset(
                    "source/target curve families changed after validation",
                ));
            }
        }
    }

    let periodic_circle =
        edges.len() == 1 && matches!(edges[0].source.curve, crate::ProfileOffsetCurve::Circle(_));
    let junction_count = if periodic_circle {
        0
    } else if closed {
        edges.len()
    } else {
        edges.len().saturating_sub(1)
    };
    for junction_index in 0..junction_count {
        let incoming = edges[junction_index];
        let branch = junctions[junction_index];
        let branch_binding = vec![
            AuditBinding::new("selected path", selected_path),
            AuditBinding::new("junction", junction_index.to_string()),
            AuditBinding::new("branch", format!("{branch:?}")),
        ];
        if branch == crate::ProfileOffsetJunctionBranch::Tangent {
            compile_profile_offset_anchor_block(
                sketch,
                problem,
                point_variables,
                circle_radius_variables,
                arc_radius_variables,
                arc_angle_variables,
                source_id,
                incoming.source,
                incoming.target,
                ProfilePathEndpoint::End,
                branch_binding,
                residual_ids,
            )?;
        }
    }

    if !closed && !periodic_circle {
        for (edge, endpoint, terminal) in [
            (edges[0], ProfilePathEndpoint::Start, "start"),
            (edges[edges.len() - 1], ProfilePathEndpoint::End, "end"),
        ] {
            compile_profile_offset_anchor_block(
                sketch,
                problem,
                point_variables,
                circle_radius_variables,
                arc_radius_variables,
                arc_angle_variables,
                source_id,
                edge.source,
                edge.target,
                endpoint,
                vec![
                    AuditBinding::new("selected path", selected_path),
                    AuditBinding::new("free terminal", terminal),
                ],
                residual_ids,
            )?;
        }
    }
    Ok(())
}

fn compile_profile_offset_source_angle_preferences(
    sketch: &Sketch,
    problem: &mut Problem,
    arc_angle_variables: &[ArcAngleVariableMapping],
    source_id: SourceConstraintId,
    source: ArcId,
    bindings: &[AuditBinding],
    residual_ids: &mut Vec<ResidualId>,
) -> Result<(), SketchError> {
    let retained = sketch.arc_value(source)?;
    for (role, endpoint, target) in [
        (ArcAngleRole::Start, "start", retained.start_angle()),
        (ArcAngleRole::End, "end", retained.end_angle()),
    ] {
        let mut row_bindings = bindings.to_owned();
        row_bindings.extend([
            AuditBinding::new("source endpoint", endpoint),
            AuditBinding::new("retained source angle", target.to_string()),
        ]);
        residual_ids.push(problem.add_residual(ResidualBlock::new(
            source_id,
            ResidualCategory::Preference,
            vec![arc_angle_variable(arc_angle_variables, source, role)?],
            1,
            vec![1.0],
            vec![audit_row_unit(
                "source endpoint angle - retained source endpoint angle".into(),
                row_bindings,
                "radian",
            )],
            ScalarTargetResidual {
                target,
                multiplier: 1.0,
            },
        )?)?);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn compile_profile_offset_radial_block(
    sketch: &Sketch,
    problem: &mut Problem,
    point_variables: &[PointVariableMapping],
    circle_radius_variables: &[CircleRadiusVariableMapping],
    arc_radius_variables: &[ArcRadiusVariableMapping],
    source_id: SourceConstraintId,
    source: crate::ProfileOffsetCurve,
    target: crate::ProfileOffsetCurve,
    radius_delta: f64,
    bindings: Vec<AuditBinding>,
    residual_ids: &mut Vec<ResidualId>,
) -> Result<(), SketchError> {
    let mut incidence = IncidenceBuilder::default();
    let (source_center, source_radius) = profile_offset_radial_incidence(
        sketch,
        point_variables,
        circle_radius_variables,
        arc_radius_variables,
        &mut incidence,
        source,
    )?;
    let (target_center, target_radius) = profile_offset_radial_incidence(
        sketch,
        point_variables,
        circle_radius_variables,
        arc_radius_variables,
        &mut incidence,
        target,
    )?;
    residual_ids.push(problem.add_residual(ResidualBlock::new(
        source_id,
        ResidualCategory::Hard,
        incidence.variables,
        3,
        vec![sketch.model_scale; 3],
        vec![
            audit_row(
                "(target.center.x - source.center.x) / model_scale".into(),
                bindings.clone(),
            ),
            audit_row(
                "(target.center.y - source.center.y) / model_scale".into(),
                bindings.clone(),
            ),
            audit_row(
                "(target.radius - source.radius - retained_radius_delta) / model_scale".into(),
                bindings,
            ),
        ],
        ProfileOffsetRadialResidual {
            source_center,
            source_radius,
            target_center,
            target_radius,
            radius_delta,
        },
    )?)?);
    Ok(())
}

#[derive(Clone, Copy)]
enum ProfilePathEndpoint {
    Start,
    End,
}

#[allow(clippy::too_many_arguments)]
fn compile_profile_offset_anchor_block(
    sketch: &Sketch,
    problem: &mut Problem,
    point_variables: &[PointVariableMapping],
    circle_radius_variables: &[CircleRadiusVariableMapping],
    arc_radius_variables: &[ArcRadiusVariableMapping],
    arc_angle_variables: &[ArcAngleVariableMapping],
    source_id: SourceConstraintId,
    source_curve: crate::DirectedProfileOffsetCurve,
    target_curve: crate::DirectedProfileOffsetCurve,
    endpoint: ProfilePathEndpoint,
    bindings: Vec<AuditBinding>,
    residual_ids: &mut Vec<ResidualId>,
) -> Result<(), SketchError> {
    let mut incidence = IncidenceBuilder::default();
    let source_join = profile_offset_endpoint_incidence(
        sketch,
        point_variables,
        circle_radius_variables,
        arc_radius_variables,
        arc_angle_variables,
        &mut incidence,
        source_curve,
        endpoint,
    )?;
    let target_join = profile_offset_endpoint_incidence(
        sketch,
        point_variables,
        circle_radius_variables,
        arc_radius_variables,
        arc_angle_variables,
        &mut incidence,
        target_curve,
        endpoint,
    )?;
    residual_ids.push(problem.add_residual(ResidualBlock::new(
        source_id,
        ResidualCategory::Hard,
        incidence.variables,
        1,
        vec![sketch.model_scale],
        vec![audit_row(
            "dot(target_join - source_join, directed_source_tangent) / model_scale".into(),
            bindings,
        )],
        ProfileOffsetTangentialAnchorResidual {
            source_join,
            source_tangent_sign: source_curve.traversal.sign(),
            target_join,
        },
    )?)?);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn profile_offset_endpoint_incidence(
    sketch: &Sketch,
    point_variables: &[PointVariableMapping],
    circle_radius_variables: &[CircleRadiusVariableMapping],
    arc_radius_variables: &[ArcRadiusVariableMapping],
    arc_angle_variables: &[ArcAngleVariableMapping],
    incidence: &mut IncidenceBuilder,
    curve: crate::DirectedProfileOffsetCurve,
    endpoint: ProfilePathEndpoint,
) -> Result<GenericCurveIncidence, SketchError> {
    let parameter = match (curve.traversal, endpoint) {
        (crate::OffsetTraversal::Forward, ProfilePathEndpoint::Start)
        | (crate::OffsetTraversal::Reverse, ProfilePathEndpoint::End) => 0.0,
        (crate::OffsetTraversal::Forward, ProfilePathEndpoint::End)
        | (crate::OffsetTraversal::Reverse, ProfilePathEndpoint::Start) => 1.0,
    };
    let curve = match curve.curve {
        crate::ProfileOffsetCurve::Line(segment) => SketchCurve::Line {
            segment,
            domain: crate::LineParameterDomain::BoundedSegment,
        },
        crate::ProfileOffsetCurve::CircularArc(arc) => SketchCurve::Arc(arc),
        crate::ProfileOffsetCurve::Circle(_) => {
            return Err(SketchError::InvalidProfileOffset(
                "a full circle has no terminal or junction endpoint",
            ));
        }
    };
    generic_curve_incidence(
        sketch,
        point_variables,
        circle_radius_variables,
        arc_radius_variables,
        arc_angle_variables,
        &[],
        &[],
        &[],
        incidence,
        curve,
        CurveParameterIncidence::Fixed(parameter),
    )
}

fn profile_offset_radial_incidence(
    sketch: &Sketch,
    point_variables: &[PointVariableMapping],
    circle_radius_variables: &[CircleRadiusVariableMapping],
    arc_radius_variables: &[ArcRadiusVariableMapping],
    incidence: &mut IncidenceBuilder,
    curve: crate::ProfileOffsetCurve,
) -> Result<(usize, usize), SketchError> {
    match curve {
        crate::ProfileOffsetCurve::CircularArc(arc) => {
            let value = sketch.arc_value(arc)?;
            Ok((
                incidence.add(point_variable(point_variables, value.center())?),
                incidence.add(arc_radius_variable(arc_radius_variables, arc)?),
            ))
        }
        crate::ProfileOffsetCurve::Circle(circle) => {
            let value = sketch.circle_value(circle)?;
            Ok((
                incidence.add(point_variable(point_variables, value.center())?),
                incidence.add(circle_radius_variable(circle_radius_variables, circle)?),
            ))
        }
        crate::ProfileOffsetCurve::Line(_) => Err(SketchError::InvalidProfileOffset(
            "a line reached the radial offset compiler",
        )),
    }
}

fn profile_offset_curve_turn(
    sketch: &Sketch,
    curve: crate::DirectedProfileOffsetCurve,
) -> Result<f64, SketchError> {
    let native = match curve.curve {
        crate::ProfileOffsetCurve::CircularArc(arc) => match sketch.arc_value(arc)?.sweep() {
            crate::ArcSweep::CounterClockwise => 1.0,
            crate::ArcSweep::Clockwise => -1.0,
        },
        crate::ProfileOffsetCurve::Circle(_) => 1.0,
        crate::ProfileOffsetCurve::Line(_) => {
            return Err(SketchError::InvalidProfileOffset(
                "a line has no radial turn branch",
            ));
        }
    };
    Ok(native * curve.traversal.sign())
}

fn profile_offset_curve_label(
    sketch: &Sketch,
    curve: crate::ProfileOffsetCurve,
) -> Result<String, SketchError> {
    Ok(match curve {
        crate::ProfileOffsetCurve::Line(segment) => sketch
            .segments
            .get(segment)
            .ok_or(SketchError::UnknownSegment(segment))?
            .label()
            .to_owned(),
        crate::ProfileOffsetCurve::CircularArc(arc) => sketch.arc_value(arc)?.label().to_owned(),
        crate::ProfileOffsetCurve::Circle(circle) => {
            sketch.circle_value(circle)?.label().to_owned()
        }
    })
}
