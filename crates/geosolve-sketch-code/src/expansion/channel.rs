// SPDX-License-Identifier: GPL-3.0-or-later

//! A patch-private recipe composed exclusively from ordinary native operations.
//!
//! Numeric geometry below selects finite initialization samples. Native
//! supporting-line Offset dimensions and computed Fillets own the geometry;
//! the existing Fillet authoring owner records the accepted explicit branches.

use super::{
    AuthoringDeclaration, BTreeMap, CodeExpansionError, CodeHostRequest, CodeInteractionOverlay,
    CodeOperationPlanner, ExpandedFeatureCorner, ExpandedPort, ExpansionBuilder, FeatureKind,
    IntentPatchOperation, IntentPortKind, IntentUnit, InvocationPlan, KeyedFilletHostRequest,
    ManagedValue, PlannedTemplateOutput, ResolvedTemplateBindings, SemanticOutputPath,
    SemanticPathMap, SemanticSymbol, SemanticValue, UnitLiteral, convert_named_unit, fields_path,
    index_path, intent_content_digest, lower_named_declaration, object, required,
    select_planned_outputs, string, unit_direction,
};

struct Vertex {
    key: String,
    point: ManagedValue,
    seed: [f64; 2],
}

struct Source {
    vertices: Vec<Vertex>,
    spans: Vec<ManagedValue>,
    closed: bool,
}

struct Recipe<'a> {
    builder: &'a mut ExpansionBuilder,
    plan: &'a InvocationPlan,
    overlay: &'a CodeInteractionOverlay,
    planner: &'a mut dyn CodeOperationPlanner,
    anchor: &'a PlannedTemplateOutput,
    outputs: &'a [PlannedTemplateOutput],
    symbols: Vec<SemanticSymbol>,
    supports: Vec<ExpandedPort>,
}

pub(super) fn lower(
    builder: &mut ExpansionBuilder,
    plan: &InvocationPlan,
    outputs: &[PlannedTemplateOutput],
    bindings: &ResolvedTemplateBindings,
    overlay: &CodeInteractionOverlay,
    planner: &mut dyn CodeOperationPlanner,
) -> Result<SemanticPathMap, CodeExpansionError> {
    let expected = [
        ("left", FeatureKind::Feature),
        ("right", FeatureKind::Feature),
        ("startLeft", FeatureKind::Point),
        ("startRight", FeatureKind::Point),
        ("endLeft", FeatureKind::Point),
        ("endRight", FeatureKind::Point),
    ];
    if outputs.len() != expected.len()
        || expected.iter().any(|(name, kind)| {
            !outputs
                .iter()
                .any(|(path, actual, _, _)| *path == fields_path(&[name]) && actual == kind)
        })
    {
        return fail(plan, "channel requires its fixed six typed outputs");
    }
    let width = scalar(builder, bindings, "width")?;
    let radius = scalar(builder, bindings, "bendRadius")?;
    if !width.is_finite() || !radius.is_finite() || width <= 0.0 || radius <= width / 2.0 {
        return fail(
            plan,
            "channel requires finite width > 0 and bendRadius > width / 2",
        );
    }
    let source = source(builder, bindings, plan)?;
    let arguments = object(&bindings.arguments, "polylineChannel")?;
    let caps = arguments
        .get("caps")
        .map_or(Ok("both"), |value| string(value, "caps"))?;
    if !["both", "end", "none"].contains(&caps) {
        return fail(plan, "channel caps must be both, end, or none");
    }
    let anchor = outputs
        .first()
        .ok_or_else(|| CodeExpansionError::Unsupported("channel has no planned output".into()))?;
    let operation_start = builder.operations.len();
    let mut recipe = Recipe {
        builder,
        plan,
        overlay,
        planner,
        anchor,
        outputs,
        symbols: Vec::new(),
        supports: Vec::new(),
    };
    let result = recipe.build(&source, width / 2.0, radius, caps);
    // Keep internal declarations available for subsequent recipe references.
    // Publish their ownership under the patch invocation only after the complete
    // recipe has been lowered.
    for operation in &recipe.builder.operations[operation_start..] {
        if let IntentPatchOperation::CreateNode { alias, .. } = operation {
            recipe
                .builder
                .declaration_provenance
                .insert(alias.clone(), plan.declaration.symbol.clone());
        }
    }
    for symbol in &recipe.symbols {
        recipe.builder.declarations.remove(symbol);
    }
    let result = result?;
    recipe
        .builder
        .host_requests
        .push(CodeHostRequest::ChannelBoundaryCheck {
            invocation: plan.declaration.symbol.clone(),
            output: anchor.2.clone(),
            identity: anchor.3,
            supports: recipe.supports,
            expected_components: if source.closed || caps == "none" {
                2
            } else {
                1
            },
            expected_open_ends: if source.closed || caps == "both" {
                0
            } else if caps == "end" {
                2
            } else {
                4
            },
            suppressed: false,
        });
    select_planned_outputs(&result, outputs, "computed.polylineChannel")
}

fn scalar(
    builder: &ExpansionBuilder,
    bindings: &ResolvedTemplateBindings,
    name: &str,
) -> Result<f64, CodeExpansionError> {
    let arguments = object(&bindings.arguments, "polylineChannel")?;
    let value = required(arguments, name, "polylineChannel")?;
    match builder.resolve_managed(value, &SemanticOutputPath::default(), name)? {
        SemanticValue::ScalarLiteral(value) => convert_named_unit(&value, IntentUnit::Length, name),
        _ => Err(CodeExpansionError::Unsupported(format!(
            "channel {name} must be a model length"
        ))),
    }
}

fn source(
    builder: &ExpansionBuilder,
    bindings: &ResolvedTemplateBindings,
    plan: &InvocationPlan,
) -> Result<Source, CodeExpansionError> {
    let value = bindings.direct.get("polyline").ok_or_else(|| {
        CodeExpansionError::Unsupported("channel polyline binding is missing".into())
    })?;
    let (SemanticValue::Declaration { alias, .. } | SemanticValue::Feature { alias, .. }) = value
    else {
        return fail(
            plan,
            "channel source must be a native keyed polyline feature",
        );
    };
    let owner = builder.declaration_provenance.get(alias).ok_or_else(|| {
        CodeExpansionError::Unsupported("channel source has no declaration owner".into())
    })?;
    let declaration = builder
        .declarations
        .get(owner)
        .ok_or_else(|| CodeExpansionError::Unsupported("channel source is unavailable".into()))?;
    let Some(SemanticValue::Collection(keyed)) = declaration.paths.get(&fields_path(&["vertices"]))
    else {
        return fail(plan, "channel source has no keyed vertices");
    };
    let mut vertices = Vec::new();
    for index in 0..keyed.len() {
        let path = index_path(&["vertices"], index, &["position"]);
        let value = declaration.paths.get(&path).ok_or_else(|| {
            CodeExpansionError::Unsupported("channel source has no ordered vertex path".into())
        })?;
        let point = value.as_port(IntentPortKind::Point, "channel source vertex")?;
        let key = keyed
            .iter()
            .find_map(|(key, value)| match value {
                SemanticValue::Port(candidate) if candidate == &point => Some(key.clone()),
                _ => None,
            })
            .ok_or_else(|| {
                CodeExpansionError::Unsupported("channel source vertex has no stable key".into())
            })?;
        let seed = builder.point_seed(value).ok_or_else(|| {
            CodeExpansionError::Unsupported(
                "channel source vertex has no finite initialization sample".into(),
            )
        })?;
        vertices.push(Vertex {
            key,
            point: reference(owner, path),
            seed,
        });
    }
    let mut spans = Vec::new();
    for index in 0..vertices.len() {
        let path = index_path(&["segments"], index, &[]);
        if let Some(value) = declaration.paths.get(&path) {
            value.as_port(IntentPortKind::CurveSpan, "channel source span")?;
            spans.push(reference(owner, path));
        }
    }
    let closed = spans.len() == vertices.len();
    if vertices.len() < 2 || (!closed && spans.len() + 1 != vertices.len()) {
        return fail(plan, "channel requires one ordered nonempty polyline");
    }
    Ok(Source {
        vertices,
        spans,
        closed,
    })
}

impl Recipe<'_> {
    fn emit(
        &mut self,
        key: &str,
        family: &str,
        arguments: ManagedValue,
    ) -> Result<SemanticSymbol, CodeExpansionError> {
        let bytes = serde_json::to_vec(&(&self.anchor.2, self.anchor.3, key))
            .map_err(|error| CodeExpansionError::Encoding(error.to_string()))?;
        let symbol = SemanticSymbol(format!("__channel_{}", intent_content_digest(&bytes)));
        let declaration = AuthoringDeclaration {
            variable: symbol.0.clone(),
            symbol: symbol.clone(),
            builder_path: family.split('.').map(str::to_owned).collect(),
            arguments,
            patch: None,
            statement_span: self.plan.declaration.statement_span,
            symbol_span: self.plan.declaration.symbol_span,
            arguments_span: self.plan.declaration.arguments_span,
        };
        lower_named_declaration(self.builder, &declaration, None, self.overlay, self.planner)?;
        self.symbols.push(symbol.clone());
        Ok(symbol)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one recipe keeps native operation order and fixed output ownership together"
    )]
    fn build(
        &mut self,
        source: &Source,
        half: f64,
        radius: f64,
        caps: &str,
    ) -> Result<SemanticPathMap, CodeExpansionError> {
        let mut walls = BTreeMap::new();
        for (side, sign) in [("left", 1.0), ("right", -1.0)] {
            walls.insert(side, self.wall(source, side, sign * half, radius)?);
        }
        let (left, left_points) = &walls["left"];
        let (right, right_points) = &walls["right"];
        if !source.closed {
            for (label, index, source_span) in [
                ("start", 0, &source.spans[0]),
                (
                    "end",
                    source.vertices.len() - 1,
                    source.spans.last().expect("source span"),
                ),
            ] {
                let cross = self.emit(
                    &format!("{label}.normal"),
                    "geometry.segment",
                    obj([
                        ("start", left_points[index].clone()),
                        ("end", right_points[index].clone()),
                        ("role", text_value("construction")),
                    ]),
                )?;
                self.emit(
                    &format!("{label}.midpoint"),
                    "constraint.midpoint",
                    obj([
                        ("point", source.vertices[index].point.clone()),
                        ("line", reference(&cross, fields_path(&["span"]))),
                    ]),
                )?;
                self.emit(
                    &format!("{label}.perpendicular"),
                    "constraint.perpendicular",
                    obj([
                        ("first", source_span.clone()),
                        ("second", reference(&cross, fields_path(&["span"]))),
                    ]),
                )?;
            }
        }
        if !source.closed {
            if caps == "both" {
                self.cap(source, "start", &left_points[0], &right_points[0], half)?;
            }
            if caps != "none" {
                self.cap(
                    source,
                    "end",
                    right_points.last().expect("wall points"),
                    left_points.last().expect("wall points"),
                    half,
                )?;
            }
        }
        let mut result = BTreeMap::new();
        for (name, symbol) in [("left", left), ("right", right)] {
            let lowered = self.builder.declarations.get(symbol).expect("emitted wall");
            let alias = match &lowered.root {
                SemanticValue::Port(port) => port.alias.clone(),
                _ => unreachable!("aggregate root is a port"),
            };
            result.insert(
                fields_path(&[name]),
                SemanticValue::Feature {
                    alias,
                    members: BTreeMap::from([("walls".to_owned(), lowered.root.clone())]),
                },
            );
        }
        for (name, value) in [
            ("startLeft", &left_points[0]),
            ("startRight", &right_points[0]),
            ("endLeft", left_points.last().expect("wall points")),
            ("endRight", right_points.last().expect("wall points")),
        ] {
            result.insert(
                fields_path(&[name]),
                self.builder
                    .resolve_managed(value, &SemanticOutputPath::default(), name)?,
            );
        }
        Ok(result)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one wall projector keeps exact source selectors and keyed bend ownership together"
    )]
    fn wall(
        &mut self,
        source: &Source,
        side: &str,
        distance: f64,
        radius: f64,
    ) -> Result<(SemanticSymbol, Vec<ManagedValue>), CodeExpansionError> {
        let positions = offset_seeds(source, distance, self.plan)?;
        let mut points = Vec::new();
        for (index, vertex) in source.vertices.iter().enumerate() {
            let point = self.emit(
                &format!("{side}.vertex.{}", vertex.key),
                "geometry.sketchPoint",
                obj([("point", sample(positions[index]))]),
            )?;
            points.push(reference(&point, fields_path(&["point"])));
        }
        let mut spans = Vec::new();
        for (index, source_span) in source.spans.iter().enumerate() {
            let line = self.emit(
                &format!("{side}.edge.{}", source.vertices[index].key),
                "geometry.segment",
                obj([
                    ("start", points[index].clone()),
                    ("end", points[(index + 1) % points.len()].clone()),
                ]),
            )?;
            let span = reference(&line, fields_path(&["span"]));
            self.supports.push(
                self.builder
                    .resolve_managed(&span, &SemanticOutputPath::default(), "channel wall")?
                    .as_port(IntentPortKind::CurveSpan, "channel wall")?,
            );
            self.emit(
                &format!("{side}.offset.{}", source.vertices[index].key),
                "dimension.supportingLineOffset",
                obj([
                    ("first", source_span.clone()),
                    ("second", span.clone()),
                    ("value", unit(distance.abs())),
                    ("side", text_value(side)),
                    ("orientation", text_value("same")),
                    ("mode", text_value("driving")),
                ]),
            )?;
            spans.push(span);
        }
        let wall = self.emit(
            &format!("{side}.wall"),
            if source.closed {
                "aggregate.closedProfile"
            } else {
                "aggregate.openChain"
            },
            obj([("spans", ManagedValue::Array(spans.clone()))]),
        )?;
        let corners = if source.closed {
            0..source.vertices.len()
        } else {
            1..source.vertices.len() - 1
        };
        for index in corners {
            let incoming = (index + source.spans.len() - 1) % source.spans.len();
            let outgoing = index % source.spans.len();
            let previous = (index + source.vertices.len() - 1) % source.vertices.len();
            let next = (index + 1) % source.vertices.len();
            let a = unit_direction(source.vertices[previous].seed, source.vertices[index].seed)
                .ok_or_else(|| {
                    CodeExpansionError::Unsupported("channel contains a zero-length span".into())
                })?;
            let b = unit_direction(source.vertices[index].seed, source.vertices[next].seed)
                .ok_or_else(|| {
                    CodeExpansionError::Unsupported("channel contains a zero-length span".into())
                })?;
            let cross = a[0] * b[1] - a[1] * b[0];
            if cross.abs() < 1e-10 {
                return fail(
                    self.plan,
                    "channel corners must have a nonzero explicit turn",
                );
            }
            let turn = cross.signum();
            let wall_radius = radius - distance * turn;
            let tangent = wall_radius * (1.0 - (a[0] * b[0] + a[1] * b[1])) / cross.abs();
            let incoming_length = length_between(positions[previous], positions[index]);
            let outgoing_length = length_between(positions[index], positions[next]);
            if tangent >= incoming_length || tangent >= outgoing_length {
                return fail(
                    self.plan,
                    "channel bend radius does not fit its adjacent spans",
                );
            }
            let output = self
                .outputs
                .iter()
                .find(|(path, _, _, _)| *path == fields_path(&[side]))
                .expect("channel side output");
            let mut member_key = output.2.member_key.clone();
            member_key.push(source.vertices[index].key.clone());
            let corner_point = self
                .builder
                .resolve_managed(
                    &points[index],
                    &SemanticOutputPath::default(),
                    "channel corner",
                )?
                .as_port(IntentPortKind::Point, "channel corner")?;
            let incoming = self
                .builder
                .resolve_managed(
                    &spans[incoming],
                    &SemanticOutputPath::default(),
                    "channel incoming",
                )?
                .as_port(IntentPortKind::CurveSpan, "channel incoming")?;
            let outgoing = self
                .builder
                .resolve_managed(
                    &spans[outgoing],
                    &SemanticOutputPath::default(),
                    "channel outgoing",
                )?
                .as_port(IntentPortKind::CurveSpan, "channel outgoing")?;
            self.builder
                .host_requests
                .push(CodeHostRequest::FilletAtCorner(KeyedFilletHostRequest {
                    invocation: self.plan.declaration.symbol.clone(),
                    member_key,
                    output: output.2.clone(),
                    identity: output.3,
                    radius: UnitLiteral {
                        unit: "mm".into(),
                        value: wall_radius,
                    },
                    corner: ExpandedFeatureCorner {
                        point: corner_point,
                        incoming,
                        outgoing,
                    },
                    artifact_digest: self.plan.pinned.digest.clone(),
                    suppressed: false,
                }));
        }
        Ok((wall, points))
    }

    fn cap(
        &mut self,
        source: &Source,
        end: &str,
        first: &ManagedValue,
        second: &ManagedValue,
        half: f64,
    ) -> Result<(), CodeExpansionError> {
        let start = end == "start";
        let index = if start { 0 } else { source.vertices.len() - 1 };
        let direction = if start {
            unit_direction(source.vertices[0].seed, source.vertices[1].seed)
        } else {
            unit_direction(source.vertices[index - 1].seed, source.vertices[index].seed)
        }
        .ok_or_else(|| {
            CodeExpansionError::Unsupported("channel cap has a zero-length source span".into())
        })?;
        let center = source.vertices[index].seed;
        let normal = [-direction[1], direction[0]];
        let sign = if start { 1.0 } else { -1.0 };
        let sample = |sign: f64| {
            ManagedValue::Array(vec![
                ManagedValue::Number(center[0] + sign * half * normal[0]),
                ManagedValue::Number(center[1] + sign * half * normal[1]),
            ])
        };
        let arc = self.emit(
            &format!("{end}.cap"),
            "geometry.centerArc",
            obj([
                ("center", source.vertices[index].point.clone()),
                ("start", sample(sign)),
                ("end", sample(-sign)),
                ("sweep", text_value("counterClockwise")),
            ]),
        )?;
        self.supports.push(
            self.builder
                .resolve(&arc, &fields_path(&["span"]))?
                .as_port(IntentPortKind::CurveSpan, "channel cap")?,
        );
        for (label, point, parameter, neighborhood) in [
            ("first", first, 0.0, "start"),
            ("second", second, 1.0, "end"),
        ] {
            self.emit(
                &format!("{end}.cap.{label}"),
                "constraint.pointOnCurve",
                obj([
                    ("point", point.clone()),
                    ("curve", reference(&arc, fields_path(&["span"]))),
                    (
                        "contact",
                        obj([
                            ("parameter", ManagedValue::Number(parameter)),
                            ("winding", ManagedValue::Number(0.0)),
                            ("neighborhood", obj([("kind", text_value(neighborhood))])),
                            ("orientation", text_value("none")),
                            (
                                "range",
                                obj([
                                    ("lower", ManagedValue::Number(parameter)),
                                    ("upper", ManagedValue::Number(parameter)),
                                ]),
                            ),
                        ]),
                    ),
                ]),
            )?;
        }
        Ok(())
    }
}

fn offset_seeds(
    source: &Source,
    distance: f64,
    plan: &InvocationPlan,
) -> Result<Vec<[f64; 2]>, CodeExpansionError> {
    let count = source.vertices.len();
    let directions = (0..source.spans.len())
        .map(|index| {
            unit_direction(
                source.vertices[index].seed,
                source.vertices[(index + 1) % count].seed,
            )
            .ok_or_else(|| {
                CodeExpansionError::Unsupported("channel contains a zero-length span".into())
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    (0..count)
        .map(|index| {
            let point = source.vertices[index].seed;
            let incoming = directions[(index + directions.len() - 1) % directions.len()];
            let outgoing = directions[index % directions.len()];
            let shift = if !source.closed && index == 0 {
                [-outgoing[1] * distance, outgoing[0] * distance]
            } else if !source.closed && index + 1 == count {
                [-incoming[1] * distance, incoming[0] * distance]
            } else {
                let denominator = 1.0 + incoming[0] * outgoing[0] + incoming[1] * outgoing[1];
                if denominator <= 1e-10 {
                    return fail(plan, "channel cannot offset a reversing corner");
                }
                [
                    -(incoming[1] + outgoing[1]) * distance / denominator,
                    (incoming[0] + outgoing[0]) * distance / denominator,
                ]
            };
            let result = [point[0] + shift[0], point[1] + shift[1]];
            if !result.iter().all(|value| value.is_finite()) {
                return fail(plan, "channel offset initialization is non-finite");
            }
            Ok(result)
        })
        .collect()
}

fn length_between(a: [f64; 2], b: [f64; 2]) -> f64 {
    (b[0] - a[0]).hypot(b[1] - a[1])
}
fn obj<const N: usize>(values: [(&str, ManagedValue); N]) -> ManagedValue {
    ManagedValue::Object(
        values
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}
fn text_value(value: &str) -> ManagedValue {
    ManagedValue::String(value.to_owned())
}
fn unit(value: f64) -> ManagedValue {
    ManagedValue::Unit(UnitLiteral {
        unit: "mm".into(),
        value,
    })
}
fn reference(symbol: &SemanticSymbol, path: SemanticOutputPath) -> ManagedValue {
    ManagedValue::Reference {
        declaration: symbol.clone(),
        path,
    }
}
fn fail<T>(plan: &InvocationPlan, message: &str) -> Result<T, CodeExpansionError> {
    Err(CodeExpansionError::InvalidDeclaration {
        declaration: plan.declaration.symbol.0.clone(),
        message: message.to_owned(),
    })
}

fn sample(position: [f64; 2]) -> ManagedValue {
    ManagedValue::Array(position.into_iter().map(ManagedValue::Number).collect())
}
