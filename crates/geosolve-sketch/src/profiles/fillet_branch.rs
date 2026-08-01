// SPDX-License-Identifier: GPL-3.0-or-later

use super::interval::{Interval, cross_interval};
use super::pieces::{PieceEvaluationError, piece_for_span};
use crate::{ContactNeighborhood, CurveSpan, SketchDocument};
use thiserror::Error;

const BRANCH_CELL_BISECTIONS: usize = 64;

/// Why a line/curve Fillet contact branch could not be enclosed without crossing a
/// tangent-parallel barrier.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum LineCurveFilletBranchCellError {
    #[error("the line and curved Fillet parents must be distinct spans")]
    SameSupport,
    #[error("the designated line Fillet parent is not an affine line or polyline span")]
    LineParentNotAffine,
    #[error("the designated curved Fillet parent is affine")]
    CurveParentAffine,
    #[error("the proposed curved-parent support bounds are invalid")]
    InvalidBounds,
    #[error("the selected curved-parent parameter is outside the proposed support bounds")]
    ParameterOutsideBounds,
    #[error("the selected Fillet parents cannot be evaluated as finite interval geometry")]
    InvalidGeometry,
    #[error("the affine Fillet parent has no certified nonzero direction")]
    ZeroLineDirection,
    #[error("the curved-parent tangent orientation at the selected root is uncertifiable")]
    UncertifiedSelectedOrientation,
    #[error("no nontrivial local Fillet branch cell could be certified")]
    UnresolvedBranchCell,
}

impl SketchDocument {
    /// Certifies one local parameter cell for the curved parent of a line/curve Fillet.
    ///
    /// `lower..upper` must be the curved span's complete bounded support or one explicit
    /// unwrapped period, and must strictly contain `parameter`. The returned open local
    /// neighborhood contains the selected parameter and cannot cross a parameter at which
    /// the curved tangent becomes parallel to the affine line. Interval derivative bounds
    /// are outward-rounded; failure is typed and does not mutate the document.
    ///
    /// # Errors
    ///
    /// Returns a typed error for incompatible parents, invalid support state, non-finite or
    /// singular geometry, or when interval analysis cannot certify a nontrivial branch cell.
    pub fn certify_line_curve_fillet_branch_cell(
        &self,
        line: CurveSpan,
        curve: CurveSpan,
        parameter: f64,
        lower: f64,
        upper: f64,
    ) -> Result<ContactNeighborhood, LineCurveFilletBranchCellError> {
        if line == curve {
            return Err(LineCurveFilletBranchCellError::SameSupport);
        }
        if !parameter.is_finite() || !lower.is_finite() || !upper.is_finite() || lower >= upper {
            return Err(LineCurveFilletBranchCellError::InvalidBounds);
        }
        if !(lower < parameter && parameter < upper) {
            return Err(LineCurveFilletBranchCellError::ParameterOutsideBounds);
        }

        let line_piece = piece_for_span(self, line).map_err(map_evaluation_error)?;
        if !line_piece.is_linear() {
            return Err(LineCurveFilletBranchCellError::LineParentNotAffine);
        }
        let curve_piece = piece_for_span(self, curve).map_err(map_evaluation_error)?;
        if curve_piece.is_linear() {
            return Err(LineCurveFilletBranchCellError::CurveParentAffine);
        }
        let line_direction = line_piece
            .derivative(Interval::point(0.5))
            .map_err(map_evaluation_error)?;
        if !line_direction[0].excludes_zero() && !line_direction[1].excludes_zero() {
            return Err(LineCurveFilletBranchCellError::ZeroLineDirection);
        }
        let selected = signed_tangent(&curve_piece, parameter, line_direction)?;
        let orientation = CertifiedSign::of(selected)
            .ok_or(LineCurveFilletBranchCellError::UncertifiedSelectedOrientation)?;

        let lower =
            certified_side_boundary(&curve_piece, line_direction, orientation, parameter, lower)?;
        let upper =
            certified_side_boundary(&curve_piece, line_direction, orientation, parameter, upper)?;
        if !(lower < parameter && parameter < upper) {
            return Err(LineCurveFilletBranchCellError::UnresolvedBranchCell);
        }
        Ok(ContactNeighborhood::Local { lower, upper })
    }
}

#[derive(Clone, Copy)]
enum CertifiedSign {
    Positive,
    Negative,
}

impl CertifiedSign {
    fn of(value: Interval) -> Option<Self> {
        if value.lower > 0.0 {
            Some(Self::Positive)
        } else if value.upper < 0.0 {
            Some(Self::Negative)
        } else {
            None
        }
    }

    fn contains(self, value: Interval) -> bool {
        match self {
            Self::Positive => value.lower > 0.0,
            Self::Negative => value.upper < 0.0,
        }
    }
}

fn certified_side_boundary(
    curve: &super::pieces::CurvePiece,
    line_direction: [Interval; 2],
    orientation: CertifiedSign,
    parameter: f64,
    support_boundary: f64,
) -> Result<f64, LineCurveFilletBranchCellError> {
    let certified = |candidate: f64| -> Result<bool, LineCurveFilletBranchCellError> {
        let tangent = curve
            .derivative(Interval::hull(parameter, candidate))
            .map_err(map_evaluation_error)?;
        Ok(orientation.contains(cross_interval(tangent, line_direction)))
    };
    if certified(support_boundary)? {
        return Ok(support_boundary);
    }

    let mut uncertified = support_boundary;
    let mut safe = parameter;
    for _ in 0..BRANCH_CELL_BISECTIONS {
        let midpoint = uncertified + 0.5 * (safe - uncertified);
        if !midpoint.is_finite()
            || midpoint.to_bits() == uncertified.to_bits()
            || midpoint.to_bits() == safe.to_bits()
        {
            break;
        }
        if certified(midpoint)? {
            safe = midpoint;
        } else {
            uncertified = midpoint;
        }
    }
    if safe.to_bits() == parameter.to_bits() {
        Err(LineCurveFilletBranchCellError::UnresolvedBranchCell)
    } else {
        Ok(safe)
    }
}

fn signed_tangent(
    curve: &super::pieces::CurvePiece,
    parameter: f64,
    line_direction: [Interval; 2],
) -> Result<Interval, LineCurveFilletBranchCellError> {
    let tangent = curve
        .derivative(Interval::point(parameter))
        .map_err(map_evaluation_error)?;
    Ok(cross_interval(tangent, line_direction))
}

const fn map_evaluation_error(_error: PieceEvaluationError) -> LineCurveFilletBranchCellError {
    LineCurveFilletBranchCellError::InvalidGeometry
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CurveDefinition, ScalarDomain, ScalarUnit};

    fn line(document: &mut SketchDocument) -> CurveSpan {
        let start = document.add_point("line start", [-10.0, 0.0]).unwrap();
        let end = document.add_point("line end", [10.0, 0.0]).unwrap();
        CurveSpan::line(
            document
                .add_curve(
                    "line",
                    CurveDefinition::Line {
                        start,
                        end,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .unwrap(),
        )
    }

    #[test]
    fn symmetric_cubic_roots_are_separated_by_the_certified_tangent_barrier() {
        let mut document = SketchDocument::new(8.0).unwrap();
        let line = line(&mut document);
        let controls = [[-6.0, 1.0], [-1.0, 8.0], [1.0, 8.0], [6.0, 1.0]]
            .map(|position| document.add_point("control", position).unwrap());
        let curve = CurveSpan::line(
            document
                .add_curve("cubic", CurveDefinition::CubicBezier { controls })
                .unwrap(),
        );
        let first_root = 0.361_804_407_541_642;
        let second_root = 0.638_195_592_458_358;

        let first = document
            .certify_line_curve_fillet_branch_cell(line, curve, first_root, 0.0, 1.0)
            .unwrap();
        let ContactNeighborhood::Local {
            lower: first_lower,
            upper: first_upper,
        } = first
        else {
            unreachable!();
        };
        assert!(first_lower <= 0.0);
        assert!(first_lower < first_root && first_root < first_upper);
        assert!(first_upper <= 0.5);
        assert!(second_root >= first_upper);

        let second = document
            .certify_line_curve_fillet_branch_cell(line, curve, second_root, 0.0, 1.0)
            .unwrap();
        let ContactNeighborhood::Local {
            lower: second_lower,
            upper: second_upper,
        } = second
        else {
            unreachable!();
        };
        assert!(second_lower >= 0.5);
        assert!(second_lower < second_root && second_root < second_upper);
        assert!(second_upper >= 1.0);
        assert!(first_root <= second_lower);
        assert_eq!(
            document.certify_line_curve_fillet_branch_cell(line, curve, 0.5, 0.0, 1.0),
            Err(LineCurveFilletBranchCellError::UncertifiedSelectedOrientation)
        );
    }

    #[test]
    fn periodic_circle_cell_uses_nearest_parallel_tangent_barriers() {
        let mut document = SketchDocument::new(4.0).unwrap();
        let line = line(&mut document);
        let center = document.add_point("center", [0.0, 3.0]).unwrap();
        let radius = document
            .add_scalar("radius", 2.0, ScalarUnit::Length, ScalarDomain::Positive)
            .unwrap();
        let circle = CurveSpan::line(
            document
                .add_curve("circle", CurveDefinition::Circle { center, radius })
                .unwrap(),
        );
        let parameter = std::f64::consts::PI;
        let neighborhood = document
            .certify_line_curve_fillet_branch_cell(
                line,
                circle,
                parameter,
                parameter - std::f64::consts::PI,
                parameter + std::f64::consts::PI,
            )
            .unwrap();
        let ContactNeighborhood::Local { lower, upper } = neighborhood else {
            unreachable!();
        };
        assert!(lower >= 0.5 * std::f64::consts::PI);
        assert!(upper <= 1.5 * std::f64::consts::PI);
        assert!(lower < parameter && parameter < upper);
    }

    #[test]
    fn same_support_failure_is_typed_and_does_not_mutate() {
        let mut document = SketchDocument::new(1.0).unwrap();
        let line = line(&mut document);
        let before = document.clone();
        assert_eq!(
            document.certify_line_curve_fillet_branch_cell(line, line, 0.5, 0.0, 1.0),
            Err(LineCurveFilletBranchCellError::SameSupport)
        );
        assert_eq!(document, before);
    }
}
