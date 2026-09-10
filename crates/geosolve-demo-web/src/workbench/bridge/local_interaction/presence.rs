// SPDX-License-Identifier: GPL-3.0-or-later
//! Disposable peer paint over the same native viewport and source ownership map.

use super::*;
use geosolve_constraint_editor::IntentNativeBinding as Binding;
use geosolve_sketch_render::{DrawFrame, DrawGeometry, DrawItem, DrawStyle};
use std::collections::BTreeSet;

const MAX_PEERS: usize = 128;
const MAX_HIGHLIGHTS: usize = 1_024;
const PALETTE: [&str; 8] = [
    "#217b8a", "#8458ba", "#bc6434", "#3c7a42", "#b64c80", "#4664b3", "#9b7528", "#597582",
];

#[derive(Clone, Default)]
pub(super) struct PresenceState {
    peers: Vec<Peer>,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Peer {
    user_id: String,
    client_id: String,
    sequence: u64,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    cursor: Option<[f64; 2]>,
    /// Exact compiler declaration symbols, resolved by the host's accepted
    /// semantic inventory. Native selection IDs are never accepted here.
    #[serde(default)]
    selection: Vec<String>,
    #[serde(default)]
    palette: Option<usize>,
}
impl PresenceState {
    pub(super) fn decode(encoded: &str, scene_key: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Request {
            scene_key: String,
            presence: Vec<Peer>,
        }
        let mut request: Request = decode_request(encoded)?;
        if request.scene_key != scene_key {
            return Err("Presence belongs to an obsolete accepted scene".into());
        }
        if request.presence.len() > MAX_PEERS {
            return Err("Presence exceeds the peer limit".into());
        }
        let mut clients = BTreeSet::new();
        let bounded_text = |text: &str| {
            !text.is_empty() && text.len() <= 128 && !text.chars().any(char::is_control)
        };
        for peer in &request.presence {
            if !bounded_text(&peer.user_id)
                || !bounded_text(&peer.client_id)
                || !clients.insert(&peer.client_id)
                || peer.sequence > MAX_SAFE_INTEGER
                || peer
                    .label
                    .as_deref()
                    .is_some_and(|text| !bounded_text(text))
                || peer
                    .cursor
                    .is_some_and(|point| !point.into_iter().all(f64::is_finite))
                || peer.selection.len() > 256
                || peer
                    .selection
                    .iter()
                    .any(|symbol| geosolve_sketch_intent::IntentKey::new(symbol).is_err())
                || peer.palette.is_some_and(|index| index >= PALETTE.len())
            {
                return Err("Invalid or excessive peer presence".into());
            }
        }
        request
            .presence
            .sort_by(|a, b| a.client_id.cmp(&b.client_id));
        Ok(Self {
            peers: request.presence,
        })
    }

    pub(super) fn append(
        &self,
        frame: &mut DrawFrame,
        scene: &EditorScene,
        policy: GeometryInteractionPolicy,
        bindings: &BTreeMap<String, Vec<Binding>>,
    ) -> Result<(), String> {
        if self.peers.is_empty() {
            return Ok(());
        }
        let mut overlay = DrawFrame {
            format: frame.format,
            view_box: frame.view_box,
            background: frame.background,
            provenance: BTreeMap::new(),
            items: Vec::new(),
        };
        self.paint(&mut overlay, scene, policy, bindings);
        overlay.validate().map_err(|error| error.to_string())?;
        frame.items.extend(overlay.items);
        Ok(())
    }
    fn paint(
        &self,
        frame: &mut DrawFrame,
        scene: &EditorScene,
        policy: GeometryInteractionPolicy,
        bindings: &BTreeMap<String, Vec<Binding>>,
    ) {
        if self.peers.is_empty() {
            return;
        }
        let mut recipients: BTreeMap<Binding, BTreeSet<usize>> = BTreeMap::new();
        let mut associations = 0;
        for (index, peer) in self.peers.iter().enumerate() {
            cursor(frame, peer, scene.viewport);
            for symbol in &peer.selection {
                if let Some(owned) = bindings.get(symbol) {
                    for binding in owned {
                        if associations == 65_536 {
                            break;
                        }
                        if recipients.entry(*binding).or_default().insert(index) {
                            associations += 1;
                        }
                    }
                }
            }
        }
        let mut remaining = (MAX_HIGHLIGHTS, 65_536);
        for point in &scene.points {
            if point.is_visible(policy)
                && let Some(peers) = recipients.get(&Binding::Point(point.id))
            {
                self.highlight(frame, peers, &mut remaining, 1, || DrawGeometry::Circle {
                    center: [point.screen_position.x, point.screen_position.y],
                    radius: 7.0,
                });
            }
        }
        for curve in &scene.curves {
            if curve.is_visible(policy)
                && let Some(peers) = recipients.get(&Binding::Curve(curve.span.curve))
            {
                self.highlight(
                    frame,
                    peers,
                    &mut remaining,
                    curve.screen_polyline.len(),
                    || DrawGeometry::Polyline {
                        points: curve
                            .screen_polyline
                            .iter()
                            .map(|point| [point.x, point.y])
                            .collect(),
                        closed: false,
                    },
                );
            }
        }
        for curve in &scene.computed_curves {
            if !curve.is_visible(policy) {
                continue;
            }
            let peers: BTreeSet<_> = [
                Binding::ComputedFeature(curve.owner.feature),
                Binding::ComputedFeatureCorner(curve.owner.corner),
            ]
            .into_iter()
            .filter_map(|binding| recipients.get(&binding))
            .flatten()
            .copied()
            .collect();
            self.highlight(
                frame,
                &peers,
                &mut remaining,
                curve.screen_polyline.len(),
                || DrawGeometry::Polyline {
                    points: curve
                        .screen_polyline
                        .iter()
                        .map(|point| [point.x, point.y])
                        .collect(),
                    closed: false,
                },
            );
        }
    }
    fn highlight(
        &self,
        frame: &mut DrawFrame,
        peers: &BTreeSet<usize>,
        remaining: &mut (usize, usize),
        points: usize,
        geometry: impl Fn() -> DrawGeometry,
    ) {
        for index in peers {
            if remaining.0 == 0 || remaining.1 < points {
                return;
            }
            remaining.0 -= 1;
            remaining.1 -= points;
            let peer = &self.peers[*index];
            push(
                frame,
                peer,
                "selection",
                geometry(),
                DrawStyle {
                    stroke: Some(color(peer).into()),
                    stroke_width: 4.0,
                    opacity: 0.45,
                    ..DrawStyle::default()
                },
            );
        }
    }
}
fn cursor(frame: &mut DrawFrame, peer: &Peer, viewport: Viewport) {
    if let Some(cursor) = peer.cursor {
        let point = viewport.model_to_screen(cursor);
        if point.x.is_finite()
            && point.y.is_finite()
            && point.x >= -20.0
            && point.y >= -20.0
            && point.x <= viewport.screen_size[0] + 20.0
            && point.y <= viewport.screen_size[1] + 20.0
        {
            let color = color(peer);
            push(
                frame,
                peer,
                "cursor",
                DrawGeometry::Polyline {
                    points: vec![
                        [point.x, point.y],
                        [point.x + 3.0, point.y + 14.0],
                        [point.x + 7.0, point.y + 9.0],
                        [point.x + 14.0, point.y + 7.0],
                    ],
                    closed: true,
                },
                DrawStyle {
                    fill: Some(color.into()),
                    stroke: Some("#ffffff".into()),
                    stroke_width: 1.5,
                    ..DrawStyle::default()
                },
            );
            push(
                frame,
                peer,
                "label",
                DrawGeometry::Text {
                    position: [point.x + 17.0, point.y + 17.0],
                    text: peer
                        .label
                        .as_deref()
                        .unwrap_or(&peer.user_id)
                        .chars()
                        .take(32)
                        .collect(),
                    rotation: 0.0,
                },
                DrawStyle {
                    fill: Some(color.into()),
                    font_family: "system-ui,sans-serif",
                    font_size: 12.0,
                    font_weight: 600,
                    ..DrawStyle::default()
                },
            );
        }
    }
}
fn color(peer: &Peer) -> &'static str {
    PALETTE[peer.palette.unwrap_or_else(|| {
        peer.user_id.bytes().fold(0_usize, |value, byte| {
            (value * 31 + usize::from(byte)) % PALETTE.len()
        })
    })]
}
fn push(frame: &mut DrawFrame, peer: &Peer, kind: &str, geometry: DrawGeometry, style: DrawStyle) {
    frame.items.push(DrawItem {
        id: format!("presence/{}/{}", peer.client_id, frame.items.len()),
        layer: "presence",
        semantic_key: None,
        class_name: format!("wb-presence-{kind}"),
        interactive: false,
        accessible_label: peer.label.clone(),
        metadata: BTreeMap::from([
            ("presenceClientId".into(), peer.client_id.clone()),
            ("presenceUserId".into(), peer.user_id.clone()),
        ]),
        style,
        geometry,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_presence_tracks_viewport_without_affecting_pick_selection_or_accepted_scene() {
        let compiled =
            geosolve_sketch_code::CompiledManagedSource::from_json(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../geosolve-sketch-engine/tests/fixtures/point-gesture-constrained.json"
            )))
            .unwrap();
        let project = geosolve_sketch_code::CodeProject::managed(
            geosolve_sketch_code::ProjectKey("presence".into()),
            compiled,
        )
        .unwrap()
        .to_canonical_json()
        .unwrap();
        let mut source = WorkbenchBridge::restore(&project).unwrap();
        let pair: serde_json::Value =
            serde_json::from_str(&source.interaction_snapshot_json().unwrap()).unwrap();
        let mut local = LocalInteraction::new(&pair["seed"].to_string()).unwrap();
        let original = source.export_project_json().unwrap();
        let before = local.compose_frame().unwrap().scene;
        let state = local.state_json().unwrap();
        let point = local.scene.points[0].screen_position;
        let pick = serde_json::json!({"x":point.x,"y":point.y}).to_string();
        let target = local.authoring_pointer_json(&pick).unwrap();
        let presence = serde_json::json!({"sceneKey":local.scene_key,"presence":[{"userId":"user-1","clientId":"tab-1","sequence":1,"label":"Alex","cursor":[5.0,0.0],"selection":["bar"],"palette":1}]});
        local.presence_json(&presence.to_string()).unwrap();
        let after = local.compose_frame().unwrap().scene;
        assert_eq!(&after.items[..before.items.len()], &before.items);
        let overlay = &after.items[before.items.len()..];
        assert_eq!(
            overlay
                .iter()
                .filter(|item| item.class_name == "wb-presence-selection")
                .count(),
            3
        );
        assert!(overlay.iter().all(|item| item.layer == "presence"
            && !item.interactive
            && item.semantic_key.is_none()));
        assert_eq!(local.state_json().unwrap(), state);
        assert_eq!(local.authoring_pointer_json(&pick).unwrap(), target);
        for invalid in [
            serde_json::json!({"sceneKey":"old","presence":[]}),
            serde_json::json!({"sceneKey":local.scene_key,"presence":vec![presence["presence"][0].clone(); 129]}),
            serde_json::json!({"sceneKey":local.scene_key,"presence":vec![presence["presence"][0].clone(); 2]}),
            serde_json::json!({"sceneKey":local.scene_key,"presence":[{"userId":"u","clientId":"c","sequence":1,"cursor":[0.0,"NaN"]}]}),
        ] {
            assert!(local.presence_json(&invalid.to_string()).is_err());
            assert_eq!(local.compose_frame().unwrap().scene, after);
        }
        local
            .wheel_json(r#"{"version":2,"x":300,"y":250,"deltaX":0,"deltaY":-90,"ctrl":false}"#)
            .unwrap();
        let zoomed = local.compose_frame().unwrap().scene;
        let cursor = zoomed
            .items
            .iter()
            .find(|item| item.class_name == "wb-presence-cursor")
            .unwrap();
        let DrawGeometry::Polyline { points, .. } = &cursor.geometry else {
            panic!("native cursor")
        };
        let expected = local.camera.viewport().model_to_screen([5.0, 0.0]);
        assert_eq!(points[0][0].to_bits(), expected.x.to_bits());
        assert_eq!(points[0][1].to_bits(), expected.y.to_bits());
        assert_ne!(
            zoomed
                .items
                .iter()
                .find(|item| item.class_name == "wb-presence-cursor"),
            after
                .items
                .iter()
                .find(|item| item.class_name == "wb-presence-cursor")
        );
        local
            .replace_json(
                &serde_json::json!({"seed":pair["seed"],"preserveSelection":true}).to_string(),
            )
            .unwrap();
        assert!(
            local
                .compose_frame()
                .unwrap()
                .scene
                .items
                .iter()
                .all(|item| item.layer != "presence")
        );
        assert_eq!(source.export_project_json().unwrap(), original);
    }
}
