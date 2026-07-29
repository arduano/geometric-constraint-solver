<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M33 CAD capability matrix

This document freezes the planar sketch capability vocabulary at M33. It is a
contract artifact, not an implementation claim and not a replacement for the
milestone order in `PLAN.md`. An `implemented_*` row identifies the milestone that
completed the public behavior; a `planned_*` row remains unavailable until that
milestone's gate passes. Frozen sketch wire languages v1-v4 are unchanged.

Marked tables are machine-readable. Cells contain no escaped pipes; identifiers are
lowercase snake case; comma-separated sets have no spaces; `none` is an explicit
value; and table row order is normative. `all_15` means every family in the curve
family table. Row emission counts are scalar hard rows for driving relations;
`0_reference` and `0_measurement` mean no equation row. A composite still owns one
persistent semantic source.

Roadmap note (2026-07-29): the machine-read `unsupported_through_m64` status and `M64` target cells
below are frozen historical M33 tokens retained for characterization compatibility. They do not
refer to an active M64 milestone; the current roadmap ends at an unscoped M62 placeholder.

## Status vocabulary

<!-- M33_TABLE:status_vocabulary:BEGIN -->
| status | target | meaning |
| --- | --- | --- |
| implemented_m32 | M32 | Public behavior implemented and released in the baseline through M32 |
| implemented_m36 | M36 | Public closed semantic operands scalar row foundations and characterization implemented at M36 |
| implemented_m37 | M37 | Public standard planar relation catalog and characterization implemented at M37 |
| implemented_m38 | M38 | Public dimensions and persistent measurements catalog implemented at M38 |
| planned_m58 | M58 | Contract target for the separate sketch-operations companion only |
| unsupported_through_m64 | M64 | Deliberately outside the supported product through M64 |
| conditional | M64 | Available only when the stated finite domain regularity branch and work conditions hold |
<!-- M33_TABLE:status_vocabulary:END -->

## Curve families

The family split follows `VisualProfileCurveFamily`. Clamped and periodic forms of
the two spline definitions are separate families, yielding exactly 15 rows.

<!-- M33_TABLE:curve_families:BEGIN -->
| id | public_definition | form | parameter_topology | status | target | reason |
| --- | --- | --- | --- | --- | --- | --- |
| line | CurveDefinition::Line | linear | bounded_or_supporting | implemented_m32 | M32 | Public v4 definition and regular common-jet support exist |
| polyline | CurveDefinition::Polyline | piecewise_linear | stable_segment | implemented_m32 | M32 | Public v4 definition exposes directed stored segments |
| circle | CurveDefinition::Circle | circular | periodic | implemented_m32 | M32 | Public v4 definition has explicit center radius and winding state |
| circular_arc | CurveDefinition::CircularArc | circular | directed_bounded | implemented_m32 | M32 | Public v4 definition has explicit sweep angles and winding state |
| ellipse | CurveDefinition::Ellipse | analytic_conic | periodic | implemented_m32 | M32 | Public v4 definition has explicit axes and regular jets |
| elliptical_arc | CurveDefinition::EllipticalArc | analytic_conic | directed_bounded | implemented_m32 | M32 | Public v4 definition has explicit axes sweep and regular jets |
| rational_quadratic_conic | CurveDefinition::RationalQuadraticConic | homogeneous_conic | bounded | implemented_m32 | M32 | Public v4 definition rejects rational poles and invalid gauges |
| parabola | CurveDefinition::ParabolaSegment | analytic_conic | trimmed_bounded | implemented_m32 | M32 | Public v4 definition has explicit vertex focus and trim |
| hyperbola | CurveDefinition::HyperbolaSegment | analytic_conic | branch_trimmed_bounded | implemented_m32 | M32 | Public v4 definition has explicit selected branch and trim |
| quadratic_bezier | CurveDefinition::QuadraticBezier | polynomial | bounded | implemented_m32 | M32 | Public v4 definition has three editable controls |
| cubic_bezier | CurveDefinition::CubicBezier | polynomial | bounded | implemented_m32 | M32 | Public v4 definition has four editable controls |
| clamped_b_spline | CurveDefinition::BSpline | clamped | stable_span | implemented_m32 | M32 | Public v4 definition has local support and semantic spans |
| periodic_b_spline | CurveDefinition::BSpline | periodic | stable_span_and_winding | implemented_m32 | M32 | Public v4 definition has cyclic controls semantic spans and winding |
| clamped_nurbs | CurveDefinition::Nurbs | clamped | stable_span | implemented_m32 | M32 | Public v4 definition has local homogeneous support and explicit weight gauge |
| periodic_nurbs | CurveDefinition::Nurbs | periodic | stable_span_and_winding | implemented_m32 | M32 | Public v4 definition has cyclic homogeneous support gauge and winding |
<!-- M33_TABLE:curve_families:END -->

## Semantic feature kinds

`family_applicability` names the curve families accepted by the feature. `design_point`
is a standalone point rather than a curve family. M36 target names describe closed
typed operands, not additions to the current public enum.

<!-- M33_TABLE:feature_kinds:BEGIN -->
| id | public_variant_or_target | value_kind | family_applicability | status | target | reason |
| --- | --- | --- | --- | --- | --- | --- |
| point | FeatureRef::Point | point | design_point | implemented_m32 | M32 | Current public feature validates one persistent design point |
| curve_endpoint | FeatureRef::CurveEndpoint | point | line,polyline,circular_arc,elliptical_arc,rational_quadratic_conic,parabola,hyperbola,quadratic_bezier,cubic_bezier,clamped_b_spline,clamped_nurbs | implemented_m32 | M32 | Current validator excludes families with periodic topology and no endpoint |
| curve_center | FeatureRef::CurveCenter | point | circle,circular_arc,ellipse,elliptical_arc,hyperbola | implemented_m32 | M32 | These definitions carry one semantic center |
| curve_axis | FeatureRef::CurveAxis | direction | line,polyline,ellipse,elliptical_arc,parabola,hyperbola | implemented_m32 | M32 | These definitions carry a semantic directed axis |
| curve_control | FeatureRef::CurveControl | point | line,polyline,quadratic_bezier,cubic_bezier | implemented_m32 | M32 | Current enum indexes directly exposed point controls only |
| curve_focus | FeatureRef::CurveFocus | point | ellipse,elliptical_arc,parabola,hyperbola | implemented_m32 | M32 | Current conic definitions expose one or two finite foci |
| fixed_curve_location | FeatureRef::FixedCurveLocation | point | all_15 | implemented_m32 | M32 | A validated contact fixes a persistent span parameter neighborhood and winding |
| conic_query_center | DocumentConicFeature::Center | point | ellipse,elliptical_arc,hyperbola | implemented_m32 | M32 | Immutable conic query returns only definitions with one semantic center |
| conic_query_focus | DocumentConicFeature::Focus | point | ellipse,elliptical_arc,parabola,hyperbola | implemented_m32 | M32 | Immutable conic query validates the family-specific focus index |
| conic_major_axis_endpoint | DocumentConicFeature::MajorAxisEndpoint | point | ellipse,elliptical_arc | implemented_m32 | M32 | Immutable conic query uses explicit major-axis orientation |
| conic_minor_axis_endpoint | DocumentConicFeature::MinorAxisEndpoint | point | ellipse,elliptical_arc | implemented_m32 | M32 | Immutable conic query uses explicit minor-axis orientation |
| conic_bounded_endpoint | DocumentConicFeature::BoundedEndpoint | point | elliptical_arc,rational_quadratic_conic,parabola,hyperbola | implemented_m32 | M32 | Immutable conic query selects one explicit bounded trim endpoint |
| conic_selected_branch_vertex | DocumentConicFeature::SelectedBranchVertex | point | hyperbola | implemented_m32 | M32 | Immutable conic query retains the explicit hyperbola branch |
| direction | DocumentDirectionRef | direction | all_15 | implemented_m36 | M36 | Closed directed operands retain axis support tangent or normal branch state independently of storage layout |
| line_support | DocumentLineSupportRef | line_support | line,polyline | implemented_m36 | M36 | Persistent semantic span plus explicit forward or reverse direction selects the supporting line |
| curve_span | DocumentCurveSpanRef | curve_span | all_15 | implemented_m36 | M36 | Stable semantic span identity and explicit winding validate without coordinate inference |
| scalar_property | DocumentScalarPropertyRef | scalar | all_15 | implemented_m36 | M36 | Persistent scalar identity carries a closed unit domain and unit-specific branch contract |
<!-- M33_TABLE:feature_kinds:END -->

## Relations

The first 23 rows exhaust `DocumentConstraintDefinition`. Specialized legacy
tangencies remain listed separately from common-jet relations because they have
different operands and branch contracts. Planned constructors allocate existing
relation state and do not introduce an undocumented residual formula.

<!-- M33_TABLE:relations:BEGIN -->
| id | public_variant_or_target | operands | unit | sign | branch_state | row_emission | status | target | reason |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| fixed_point | DocumentConstraintDefinition::FixedPoint | point,target_point | length | signed_cartesian | none | 2_hard | implemented_m32 | M32 | Fixes both Cartesian coordinates through trusted fixed-variable rows |
| fixed_coordinate | DocumentConstraintDefinition::FixedCoordinate | point,axis,target | length | signed_cartesian | coordinate_axis | 1_hard | implemented_m32 | M32 | Fixes one selected Cartesian coordinate |
| coincident | DocumentConstraintDefinition::Coincident | point,point | length | signed_cartesian | none | 2_hard | implemented_m32 | M32 | Equates two distinct persistent points |
| horizontal_line | DocumentConstraintDefinition::Horizontal | line_support | length | signed_difference | directed_line_span | 1_hard | implemented_m32 | M32 | Applies only to a validated line or polyline segment support |
| vertical_line | DocumentConstraintDefinition::Vertical | line_support | length | signed_difference | directed_line_span | 1_hard | implemented_m32 | M32 | Applies only to a validated line or polyline segment support |
| point_on_curve | DocumentConstraintDefinition::PointOnCurve | point,curve_contact | length | signed_cartesian | domain,span,winding,neighborhood | 2_hard | implemented_m32 | M32 | Common-jet position rows cover every family |
| parallel_lines | DocumentConstraintDefinition::Parallel | line_support,line_support | dimensionless | orientation_invariant | none | 1_hard | implemented_m32 | M32 | Unit-direction cross product accepts parallel or antiparallel supports |
| perpendicular_lines | DocumentConstraintDefinition::Perpendicular | line_support,line_support | dimensionless | orientation_invariant | none | 1_hard | implemented_m32 | M32 | Unit-direction dot product applies only to line supports |
| equal_segment_length | DocumentConstraintDefinition::EqualLength | line_support,line_support | length | unsigned_magnitude | none | 1_hard | implemented_m32 | M32 | Compares finite segment lengths and is not generic path length |
| equal_circle_radius | DocumentConstraintDefinition::EqualRadius | circle,circle | length | positive_magnitude | none | 1_hard | implemented_m32 | M32 | Current lowering deliberately requires two full circles |
| midpoint_on_line | DocumentConstraintDefinition::Midpoint | point,line_support | length | signed_cartesian | directed_line_span | 2_hard | implemented_m32 | M32 | Equates a point to the arithmetic midpoint of one segment |
| symmetric_points_about_line | DocumentConstraintDefinition::SymmetricAboutLine | point,point,line_support | length | signed_cartesian | directed_line_span | 2_hard | implemented_m32 | M32 | One source emits midpoint-on-axis and pair-normal rows |
| line_circle_tangency | DocumentConstraintDefinition::LineCircleTangency | line_contact,circle_contact | mixed_length_dimensionless | side_selected | side,domain,neighborhood,winding,tangent_orientation | 3_hard | implemented_m32 | M32 | Specialized contact emits two position rows and one tangent row |
| circle_circle_tangency | DocumentConstraintDefinition::CircleCircleTangency | circle,circle | length | branch_selected | external_or_internal,containment,center_direction | 1_hard | implemented_m32 | M32 | Center-distance equation has an explicit containment and direction branch |
| circle_arc_tangency | DocumentConstraintDefinition::CircleArcTangency | circle_contact,arc_contact | mixed_length_dimensionless | side_selected | radial_side,span,winding,neighborhood,tangent_orientation | 3_hard | implemented_m32 | M32 | Specialized bounded contact emits two position rows and one tangent row |
| line_curve_tangency | DocumentConstraintDefinition::LineCurveTangency | line_endpoint,curve_contact | mixed_length_dimensionless | orientation_selected | endpoint,span,winding,neighborhood,tangent_orientation | 3_hard | implemented_m32 | M32 | Line endpoint and common-jet contact share position and tangent rows |
| curve_curve_contact | DocumentConstraintDefinition::CurveCurveContact | curve_contact,curve_contact | length | signed_cartesian | domain,span,winding,neighborhood | 2_hard | implemented_m32 | M32 | Common-jet position rows cover every regular family pair |
| curve_curve_tangency | DocumentConstraintDefinition::CurveCurveTangency | curve_contact,curve_contact | mixed_length_dimensionless | orientation_selected | domain,span,winding,neighborhood,tangent_orientation | 3_hard | implemented_m32 | M32 | Contact rows plus one oriented tangent row cover every regular family pair |
| curve_direction | DocumentConstraintDefinition::CurveDirection | line_support,curve_contact | dimensionless | direction_selected | tangent_orientation_or_normal_side,span,winding,neighborhood | 1_hard | implemented_m32 | M32 | Constrains a directed line to a selected tangent or sided normal |
| equal_curvature | DocumentConstraintDefinition::EqualCurvature | curve_contact,curve_contact | dimensionless | signed_or_magnitude_branch | curvature_relation,span,winding,neighborhood | 1_hard | implemented_m32 | M32 | Model-scaled signed equation retains explicit magnitude sign relation |
| endpoint_continuity | DocumentConstraintDefinition::EndpointContinuity | endpoint_contact,endpoint_contact | mixed_length_dimensionless | ordered_path | endpoint_order,span,winding,continuity,rates | G0=2,G1=3,G2=4,ParametricC2=6 | implemented_m32 | M32 | Ordered common jets implement separately named geometric and parametric continuity |
| line_line_fillet | DocumentConstraintDefinition::LineLineFillet | output_arc,line_contact,line_contact | mixed_length_dimensionless | side_selected | two_normal_sides,endpoint_order,sweep,neighborhood | 6_hard | implemented_m32 | M32 | One association emits four offset-center rows and two output-radial rows |
| curve_curve_fillet | DocumentConstraintDefinition::CurveCurveFillet | output_arc,curve_contact,curve_contact | mixed_length_dimensionless | side_selected | two_normal_sides,two_trim_endpoints,endpoint_order,sweep,span,winding,neighborhood | 6_hard | implemented_m32 | M32 | Common-jet association supports all regular family pairs with explicit trim ownership |
| fixed_scalar | DocumentScalarRelation::Fixed | scalar_property,target | property_unit | property_defined | scalar_domain | 1_hard | implemented_m36 | M36 | One catalog-owned persistent semantic source lowers one deterministic raw/normalized row with complete audit and independent evidence validation |
| equal_scalar | DocumentScalarRelation::Equal | scalar_property,scalar_property | property_unit | property_defined | scalar_domain | 1_hard | implemented_m36 | M36 | Exact unit domain support and neighborhood compatibility is validated before one deterministic equality row is emitted |
| concentric | concentric_relation | center_feature,center_feature | length | signed_cartesian | none | 2_hard | implemented_m37 | M37 | Only center-bearing families are applicable |
| collinear | collinear_relation | line_support,line_support | mixed_length_dimensionless | orientation_invariant | none | 2_hard | implemented_m37 | M37 | Parallel direction and zero signed support offset form one semantic source |
| horizontal_points | horizontal_point_pair_relation | point,point | length | signed_difference | none | 1_hard | implemented_m37 | M37 | Arbitrary point features no longer require a line entity |
| vertical_points | vertical_point_pair_relation | point,point | length | signed_difference | none | 1_hard | implemented_m37 | M37 | Arbitrary point features no longer require a line entity |
| block_entity | block_entity_relation | entity | mixed | property_defined | captured_semantic_state | variable_hard | implemented_m37 | M37 | One grouped source fixes every independent semantic degree of the selected entity |
| point_symmetry_about_center | point_symmetry_relation | point,point,center_point | length | signed_cartesian | none | 2_hard | implemented_m37 | M37 | Pair midpoint is constrained to the explicit center feature |
| entity_symmetry_about_line | entity_symmetry_relation | entity,entity,line_support | mixed | correspondence_selected | operand_correspondence,line_orientation | variable_hard | implemented_m37 | M37 | Entity correspondence is explicit and never inferred by proximity |
| equal_circular_radius | equal_circular_radius_relation | circular_curve,circular_curve | length | positive_magnitude | none | 1_hard | implemented_m37 | M37 | Generalizes current circle-only equality to circles and circular arcs |
| equal_distance | equal_distance_relation | point_pair,point_pair | length | unsigned_magnitude | none | 1_hard | implemented_m37 | M37 | Two semantic point-pair distances share one equality row |
| equal_angle | equal_angle_relation | angle_operand,angle_operand | angle | directed | orientation,winding | 1_hard | implemented_m37 | M37 | Both angle operands carry explicit orientation and unwrapping state |
| contact_constructor | high_level_contact_constructor | point_or_curve_operands | length | property_defined | allocated_domain,span,winding,neighborhood | delegates_2_hard | implemented_m37 | M37 | Constructor allocates explicit latent contact state then emits the existing contact relation |
| tangent_constructor | high_level_tangent_constructor | curve_operands | mixed_length_dimensionless | orientation_selected | allocated_domain,span,winding,neighborhood,tangent_orientation,side_or_containment | delegates_3_hard | implemented_m37 | M37 | Constructor allocates branch state then emits the existing tangent relation |
| equal_path_length | equal_path_length_relation | bounded_curve_interval,bounded_curve_interval | length | unsigned_magnitude | interval_endpoints,span,winding | 1_hard | implemented_m38 | M38 | Equality is permitted only with bounded value derivative and work evidence |
<!-- M33_TABLE:relations:END -->

## Dimensions and measurements

The first seven rows exhaust `DocumentDimensionDefinition`. The next nine exhaust
the current public differential and conic measurement enums. Current conic queries
are equation-free and are not yet persistent measurement objects. All dimension
targets remain one semantic source in both driving and reference mode.

<!-- M33_TABLE:dimensions_measurements:BEGIN -->
| id | public_variant_or_target | kind | operands | unit | sign | branch_state | row_emission | status | target | reason |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| dimension_point_distance | DocumentDimensionDefinition::PointDistance | dimension | point,point | length | unsigned_positive | none | driving=1,reference=0 | implemented_m32 | M32 | Current target scalar is positive length |
| dimension_curve_length | DocumentDimensionDefinition::CurveLength | dimension | line_support | length | unsigned_positive | directed_line_span | driving=1,reference=0 | implemented_m32 | M32 | Despite its name current v4 meaning is segment length only |
| dimension_radius | DocumentDimensionDefinition::Radius | dimension | circle_or_circular_arc | length | positive_magnitude | none | driving=1,reference=0 | implemented_m32 | M32 | Current lowering accepts only circle and circular arc radius variables |
| dimension_diameter | DocumentDimensionDefinition::Diameter | dimension | circle_or_circular_arc | length | positive_magnitude | none | driving=1,reference=0 | implemented_m32 | M32 | Current lowering emits twice the circular radius |
| dimension_oriented_angle | DocumentDimensionDefinition::OrientedAngle | dimension | line_support,line_support | angle | directed_positive | clockwise_or_counterclockwise | driving=1,reference=0 | implemented_m32 | M32 | Target-relative unwrap preserves explicit angle orientation |
| dimension_supporting_line_offset | DocumentDimensionDefinition::SupportingLineOffset | dimension | line_support,line_support | length | positive_magnitude_with_side | side,endpoint_orientation | driving=2,reference=0 | implemented_m32 | M32 | Driving form emits parallelism and signed normal separation rows |
| dimension_exact_translated_segment_offset | DocumentDimensionDefinition::ExactTranslatedSegmentOffset | dimension | line_support,line_support | length | positive_magnitude_with_side | side,endpoint_orientation | driving=4,reference=0 | implemented_m32 | M32 | Driving form emits exact endpoint translation rows |
| measurement_signed_curvature | DocumentCurveMeasurementKind::SignedCurvature | measurement | curve_contact | curvature | signed_by_parameter_orientation | span,winding,neighborhood | 0_measurement | implemented_m32 | M32 | Fresh common-jet differential evaluation is equation-free |
| measurement_unsigned_curvature | DocumentCurveMeasurementKind::UnsignedCurvature | measurement | curve_contact | curvature | unsigned_nonnegative | span,winding,neighborhood | 0_measurement | implemented_m32 | M32 | Fresh common-jet differential evaluation is equation-free |
| measurement_osculating_radius | DocumentCurveMeasurementKind::OsculatingRadius | measurement | curve_contact | length | positive_magnitude | span,winding,neighborhood | 0_measurement | implemented_m32 | M32 | Straight or zero-curvature locations return typed undefined output |
| measurement_conic_major_axis_length | DocumentConicMeasurement::MajorAxisLength | measurement | ellipse_or_elliptical_arc | length | positive_magnitude | axis_observability | 0_measurement | implemented_m32 | M32 | Immutable conic query with no persistent measurement identity |
| measurement_conic_minor_axis_length | DocumentConicMeasurement::MinorAxisLength | measurement | ellipse_or_elliptical_arc | length | positive_magnitude | axis_observability | 0_measurement | implemented_m32 | M32 | Immutable conic query with no persistent measurement identity |
| measurement_conic_linear_eccentricity | DocumentConicMeasurement::LinearEccentricity | measurement | ellipse_or_elliptical_arc | length | unsigned_nonnegative | axis_observability | 0_measurement | implemented_m32 | M32 | Immutable conic query with no persistent measurement identity |
| measurement_conic_focal_distance | DocumentConicMeasurement::FocalDistance | measurement | parabola_or_hyperbola | length | positive_magnitude | hyperbola_branch | 0_measurement | implemented_m32 | M32 | Immutable conic query with no persistent measurement identity |
| measurement_conic_transverse_axis_length | DocumentConicMeasurement::TransverseAxisLength | measurement | hyperbola | length | positive_magnitude | hyperbola_branch | 0_measurement | implemented_m32 | M32 | Immutable selected-hyperbola query with no persistent measurement identity |
| measurement_conic_conjugate_axis_length | DocumentConicMeasurement::ConjugateAxisLength | measurement | hyperbola | length | positive_magnitude | hyperbola_branch | 0_measurement | implemented_m32 | M32 | Immutable selected-hyperbola query with no persistent measurement identity |
| dimension_relative_horizontal | relative_horizontal_dimension | dimension | point,point | length | signed | point_order | driving=1,reference=0 | implemented_m38 | M38 | Ordered point operands define the sign without coordinate inference |
| dimension_relative_vertical | relative_vertical_dimension | dimension | point,point | length | signed | point_order | driving=1,reference=0 | implemented_m38 | M38 | Ordered point operands define the sign without coordinate inference |
| dimension_datum_coordinate | datum_coordinate_dimension | dimension | point,datum_axis | length | signed | coordinate_axis,datum_identity | driving=1,reference=0 | implemented_m38 | M38 | Absolute coordinate is relative to an explicit datum rather than an implicit UI origin |
| dimension_point_line_distance | point_line_distance_dimension | dimension | point,line_support | length | signed | line_orientation,side | driving=1,reference=0 | implemented_m38 | M38 | Directed support and side define one smooth signed distance |
| dimension_parallel_line_separation | parallel_line_separation_dimension | dimension | line_support,line_support | length | signed | support_order,orientation,side | driving=2,reference=0 | implemented_m38 | M38 | Driving form includes parallelism and signed separation in one source |
| dimension_two_line_angle | two_line_angle_dimension | dimension | line_support,line_support | angle | directed | orientation,winding | driving=1,reference=0 | implemented_m38 | M38 | Ordered directed supports retain angle branch state |
| dimension_three_point_angle | three_point_angle_dimension | dimension | point,vertex,point | angle | directed | endpoint_order,orientation,winding | driving=1,reference=0 | implemented_m38 | M38 | Vertex and ray order are semantic operands |
| dimension_circular_sweep | circular_sweep_dimension | dimension | circular_arc | angle | directed | sweep,winding | driving=1,reference=0 | implemented_m38 | M38 | Sweep uses explicit traversal rather than endpoint-only inference |
| dimension_circular_arc_length | circular_arc_length_dimension | dimension | circular_arc | length | positive_magnitude | sweep,winding | driving=1,reference=0 | implemented_m38 | M38 | Radius and explicit sweep define the same driving and reference value |
| dimension_ellipse_major_axis | ellipse_major_axis_dimension | dimension | ellipse_or_elliptical_arc | length | positive_magnitude | axis_observability | driving=1,reference=0 | implemented_m38 | M38 | Uses the same immutable property as the current query |
| dimension_ellipse_minor_axis | ellipse_minor_axis_dimension | dimension | ellipse_or_elliptical_arc | length | positive_magnitude | axis_observability | driving=1,reference=0 | implemented_m38 | M38 | Uses the same immutable property as the current query |
| dimension_conic_linear_eccentricity | conic_linear_eccentricity_dimension | dimension | ellipse_or_elliptical_arc | length | unsigned_nonnegative | axis_observability | driving=1,reference=0 | implemented_m38 | M38 | Supported conic property receives persistent driving and reference forms |
| dimension_conic_focal_distance | conic_focal_distance_dimension | dimension | parabola_or_hyperbola | length | positive_magnitude | hyperbola_branch | driving=1,reference=0 | implemented_m38 | M38 | Supported conic property receives persistent driving and reference forms |
| dimension_conic_transverse_axis_length | conic_transverse_axis_length_dimension | dimension | hyperbola | length | positive_magnitude | hyperbola_branch | driving=1,reference=0 | implemented_m38 | M38 | Supported selected-branch property receives persistent forms |
| dimension_conic_conjugate_axis_length | conic_conjugate_axis_length_dimension | dimension | hyperbola | length | positive_magnitude | hyperbola_branch | driving=1,reference=0 | implemented_m38 | M38 | Supported selected-branch property receives persistent forms |
| measurement_persistent_signed_curvature | persistent_signed_curvature_measurement | measurement | curve_contact | curvature | signed_by_parameter_orientation | span,winding,neighborhood,provenance | 0_measurement | implemented_m38 | M38 | Persists typed identity provenance and accepted input stamp |
| measurement_persistent_unsigned_curvature | persistent_unsigned_curvature_measurement | measurement | curve_contact | curvature | unsigned_nonnegative | span,winding,neighborhood,provenance | 0_measurement | implemented_m38 | M38 | Persists typed identity provenance and accepted input stamp |
| measurement_persistent_osculating_radius | persistent_osculating_radius_measurement | measurement | curve_contact | length | positive_magnitude | span,winding,neighborhood,provenance | 0_measurement | implemented_m38 | M38 | Persists typed identity and typed undefined curvature outcomes |
| measurement_bounded_curve_length | bounded_curve_length_measurement | measurement | bounded_curve_interval | length | unsigned_nonnegative | interval_endpoints,span,winding,provenance | 0_measurement | implemented_m38 | M38 | Equation-free value requires certified finite integration work |
| dimension_path_length | path_length_dimension | dimension | bounded_curve_interval | length | positive_magnitude | interval_endpoints,span,winding | driving=1,reference=0 | implemented_m38 | M38 | Driving form requires bounded value and derivative work before emission |
| dimension_segment_length | segment_length_dimension | dimension | line_support | length | positive_magnitude | directed_line_span | driving=1,reference=0 | implemented_m38 | M38 | Migration-safe name replaces the misleading line-only CurveLength meaning |
<!-- M33_TABLE:dimensions_measurements:END -->

## Scalar units and domains

GeoSolve stores canonical numeric units only. Authored/display units and conversion
remain host-owned. Signed length uses the existing `Length` plus finite domain and a
semantic sign convention; it is not a separate physical dimension.

<!-- M33_TABLE:scalar_units:BEGIN -->
| id | public_variant_or_target | quantity | sign_policy | branch_state | row_emission | status | target | reason |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| length | ScalarUnit::Length | length | domain_and_semantic_role | none | property_defined | implemented_m32 | M32 | Canonical model-unit length exists in v4 |
| angle | ScalarUnit::Angle | angle | orientation_and_unwrap | orientation,winding | property_defined | implemented_m32 | M32 | Canonical numeric angle is radians |
| parameter | ScalarUnit::Parameter | curve_parameter | domain_defined | span,winding,neighborhood | property_defined | implemented_m32 | M32 | Curve parameter value is not a model length |
| dimensionless | DocumentScalarUnit::Dimensionless | dimensionless | property_defined | none | property_defined | implemented_m36 | M36 | Explicit operand semantics distinguish ratios from frozen-v4 parameter storage |
| curvature | DocumentScalarUnit::Curvature | inverse_length | signed_by_parameter_orientation | normal_side_or_sign_relation | property_defined | implemented_m36 | M36 | Distinct inverse-length scaling and explicit signed or normal-side state are validated |
| signed_length_semantics | DocumentScalarBranch::SignedLength | length | signed | operand_order,axis,side_or_datum | property_defined | implemented_m36 | M36 | Closed provenance records ordered axis side or datum meaning without inspecting coordinates |
<!-- M33_TABLE:scalar_units:END -->

<!-- M33_TABLE:scalar_domains:BEGIN -->
| id | public_variant | bounds | sign_policy | branch_state | row_emission | status | target | reason |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| finite | ScalarDomain::Finite | finite_only | signed_or_unsigned_by_role | none | 0_domain | implemented_m32 | M32 | Any nonfinite scalar is rejected before solve or publication |
| positive | ScalarDomain::Positive | value_gt_zero | positive | none | 0_domain | implemented_m32 | M32 | Radius and positive magnitude targets use an explicit lower domain |
| bounded | ScalarDomain::Bounded | inclusive_lower_upper | signed_or_unsigned_by_role | active_bound | 0_domain | implemented_m32 | M32 | Finite ordered limits participate in active-bound semantics |
| periodic | ScalarDomain::Periodic | positive_period | wrapped_principal_value | winding | 0_domain | implemented_m32 | M32 | Winding remains explicit state outside the scalar value |
<!-- M33_TABLE:scalar_domains:END -->

## Unsupported and conditional combinations

These rows are normative exclusions or preconditions. A missing combination must not
be inferred from coordinate similarity, a convenient initial guess, tessellation, or
a target row elsewhere in this document.

<!-- M33_TABLE:unsupported_combinations:BEGIN -->
| id | capability | operands | status | target | reason |
| --- | --- | --- | --- | --- | --- |
| periodic_curve_endpoint | endpoint_feature | circle,ellipse,periodic_b_spline,periodic_nurbs | unsupported_through_m64 | M64 | Periodic topology has no distinguished start or end feature |
| center_without_semantic_center | center_feature | line,polyline,rational_quadratic_conic,parabola,quadratic_bezier,cubic_bezier,clamped_b_spline,periodic_b_spline,clamped_nurbs,periodic_nurbs | unsupported_through_m64 | M64 | These definitions carry no unique semantic center |
| focus_without_semantic_focus | focus_feature | line,polyline,circle,circular_arc,rational_quadratic_conic,quadratic_bezier,cubic_bezier,clamped_b_spline,periodic_b_spline,clamped_nurbs,periodic_nurbs | unsupported_through_m64 | M64 | These definitions carry no unique conic focus |
| axis_without_semantic_axis | axis_feature | circle,circular_arc,rational_quadratic_conic,quadratic_bezier,cubic_bezier,clamped_b_spline,periodic_b_spline,clamped_nurbs,periodic_nurbs | unsupported_through_m64 | M64 | An axis cannot be selected from incidental coordinates |
| spline_control_feature_current_gap | DocumentControlRef | clamped_b_spline,periodic_b_spline,clamped_nurbs,periodic_nurbs | implemented_m36 | M36 | The capability-specific operand uses owning curve plus persistent point membership and survives knot insertion; legacy FeatureRef retains its frozen index language |
| public_curve_plugin | curve_family | arbitrary_trait_object | unsupported_through_m64 | M64 | Built-in and external snapshot curve languages remain closed and serializable |
| implicit_coefficient_conic | curve_family | implicit_polynomial_coefficients | unsupported_through_m64 | M64 | Unnormalized implicit coefficient gauges are outside the explicit parametric model |
| spatial_sketch_curve | curve_family | three_dimensional_curve | unsupported_through_m64 | M64 | Sketch geometry remains planar and host projection is external input |
| contact_invalid_domain | point_or_curve_contact | escaped_parameter,ambiguous_neighborhood,invalid_span | conditional | M64 | Contact succeeds only for the explicit valid domain span winding and neighborhood |
| tangency_zero_speed | tangent_relation | zero_speed_or_nondifferentiable_jet | conditional | M64 | A finite nonzero first derivative is required for tangent orientation |
| curvature_insufficient_regularity | curvature_relation_or_measurement | zero_speed_or_missing_second_derivative | conditional | M64 | Curvature needs a regular second-order jet and finite conditioning |
| c2_insufficient_regularity | parametric_c2_relation | span_without_guaranteed_c2 | conditional | M64 | Parametric C2 cannot be claimed across an insufficiently continuous knot |
| fillet_parallel_parents | fillet_relation | parallel_or_unresolved_parent_tangents | conditional | M64 | The selected local offset intersection is not uniquely regular |
| fillet_singular_offset | fillet_relation | one_minus_side_radius_curvature_near_zero | conditional | M64 | The selected normal offset is singular or numerically unresolved |
| radius_non_circular | radius_dimension | ellipse,elliptical_arc,rational_quadratic_conic,parabola,hyperbola,quadratic_bezier,cubic_bezier,clamped_b_spline,periodic_b_spline,clamped_nurbs,periodic_nurbs | unsupported_through_m64 | M64 | Noncircular curves do not have one CAD radius property |
| diameter_non_circular | diameter_dimension | ellipse,elliptical_arc,rational_quadratic_conic,parabola,hyperbola,quadratic_bezier,cubic_bezier,clamped_b_spline,periodic_b_spline,clamped_nurbs,periodic_nurbs | unsupported_through_m64 | M64 | Axis and conic-property dimensions replace an ambiguous diameter |
| equal_radius_non_circular | equal_circular_radius_relation | ellipse,elliptical_arc,rational_quadratic_conic,parabola,hyperbola,quadratic_bezier,cubic_bezier,clamped_b_spline,periodic_b_spline,clamped_nurbs,periodic_nurbs | unsupported_through_m64 | M64 | Equal circular radius applies only to circles and circular arcs |
| concentric_without_centers | concentric_relation | any_operand_without_curve_center | unsupported_through_m64 | M64 | Concentricity requires two explicit semantic center features |
| collinear_non_linear | collinear_relation | any_non_line_support | unsupported_through_m64 | M64 | Collinearity is a relation between explicit supporting lines |
| horizontal_whole_non_linear | horizontal_relation | any_non_line_support | unsupported_through_m64 | M64 | Whole-curve horizontal is ambiguous and a contact tangent direction must be used instead |
| vertical_whole_non_linear | vertical_relation | any_non_line_support | unsupported_through_m64 | M64 | Whole-curve vertical is ambiguous and a contact tangent direction must be used instead |
| parallel_whole_non_linear | parallel_relation | any_non_line_support | unsupported_through_m64 | M64 | Parallelism needs line supports and not sampled curve chords |
| perpendicular_whole_non_linear | perpendicular_relation | any_non_line_support | unsupported_through_m64 | M64 | Perpendicularity needs line supports or explicit contact directions |
| midpoint_non_linear | midpoint_relation | any_non_line_support | unsupported_through_m64 | M64 | Parametric half value and half arc length are not interchangeable midpoint meanings |
| symmetry_axis_non_linear | symmetry_relation | non_line_axis | unsupported_through_m64 | M64 | Reflection requires an explicit line support and not a sampled curve tangent |
| equal_length_non_linear_current_gap | equal_length_relation | bounded_non_line_curve_interval | unsupported_through_m64 | M64 | Frozen legacy EqualLength is not broadened; implemented M38 EqualPathLength owns bounded-curve equality |
| curve_length_non_linear_current_gap | curve_length_dimension | bounded_non_line_curve_interval | unsupported_through_m64 | M64 | Frozen v4 CurveLength remains line-only; implemented M38 PathLength owns bounded non-line intervals |
| generic_curve_angle | angle_dimension | tangent_of_arbitrary_curve,tangent_of_arbitrary_curve | unsupported_through_m64 | M64 | M38 angles are two-line or three-point and curve tangents require explicit contact operands |
| arbitrary_curve_offset | offset_dimension | any_non_line_support | unsupported_through_m64 | M64 | Only supporting-line and exact translated-segment offsets are solver dimensions |
| rational_conic_property_dimension | conic_property_dimension | rational_quadratic_conic | unsupported_through_m64 | M64 | A generic rational segment has no canonical ellipse parabola or hyperbola property identity |
| driving_curvature | curvature_dimension | curve_contact,target_curvature | unsupported_through_m64 | M64 | M38 persists curvature measurements but does not promise a driving curvature dimension |
| path_length_unbounded | path_length_dimension_or_relation | supporting_line_or_unbounded_interval | conditional | M64 | Path length requires an explicit finite bounded interval |
| path_length_invalid_derivative | path_length_dimension_or_relation | pole,zero_speed,nonfinite_derivative | conditional | M64 | Value and derivative evaluation must both be finite and complete |
| path_length_work_exhausted | path_length_dimension_or_relation | exhausted_integration_or_derivative_budget | conditional | M64 | Work exhaustion is a typed non-success outcome and emits no accepted row |
| arbitrary_multi_fragment_trim | trim_view | multiple_visible_intervals_per_support_span | planned_m58 | M58 | M58 generalizes visible topology through the separate operations companion |
| solid_or_brep_operand | sketch_relation_or_dimension | face,edge,surface,solid | unsupported_through_m64 | M64 | B-rep topology and projection remain host or companion concerns rather than sketch equations |
<!-- M33_TABLE:unsupported_combinations:END -->

## Ownership consequences

- The host owns authored/display units, conversion, expressions, projection, PDM
  identity, configurations, application undo and cross-system transactions.
- GeoSolve owns canonical finite values, closed typed operands, explicit branch
  state, row emission, independent validation, and accepted measurement provenance.
- Construction macros and future companion operations may create ordinary public
  transactions but may not own private residual formulas.
- No row in this matrix adds a current API, changes `SKETCH_DOCUMENT_VERSION`, or
  authorizes draft-v5 syntax.
