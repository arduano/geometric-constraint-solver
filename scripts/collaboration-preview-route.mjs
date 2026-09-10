// SPDX-License-Identifier: GPL-3.0-or-later
/** Authoring preview is a disposable computation route, never accepted mutation. */
export function previewFault(code, message, httpStatus = 400) {
  return Object.assign(Error(message), { code, httpStatus });
}
function exact(value, keys) {
  if (!value || typeof value !== "object" || Array.isArray(value) || Object.keys(value).some(key => !keys.includes(key))) throw previewFault("invalid_preview", "Unknown or malformed preview fields");
}
function counter(value, minimum = 0) { return Number.isSafeInteger(value) && value >= minimum; }
export function validatePreviewRequest(value) {
  exact(value, value?.action === "begin"
    ? ["action", "basis", "kind", "gestureId", "viewport", "target", "tool", "role"]
    : value?.action === "advance" ? ["action", "ticket", "samples"] : ["action", "ticket"]);
  if (value.action === "begin") {
    exact(value.basis, ["documentEpoch", "revision", "sourceDesignDigest"]);
    if (!counter(value.basis.revision) || typeof value.basis.documentEpoch !== "string" || !value.basis.documentEpoch.length || value.basis.documentEpoch.length > 256
      || typeof value.basis.sourceDesignDigest !== "string" || !value.basis.sourceDesignDigest.length || value.basis.sourceDesignDigest.length > 256
      || !counter(value.gestureId, 1)) throw previewFault("invalid_preview", "Expected exact accepted basis and gesture identity");
    exact(value.viewport, ["screen_size", "model_center", "pixels_per_model_unit"]);
    const pair = input => Array.isArray(input) && input.length === 2 && input.every(Number.isFinite);
    if (!pair(value.viewport.screen_size) || !value.viewport.screen_size.every(n => n > 0)
      || !pair(value.viewport.model_center) || !Number.isFinite(value.viewport.pixels_per_model_unit) || value.viewport.pixels_per_model_unit <= 0) throw previewFault("invalid_preview", "Expected finite positive authoring viewport");
    if (value.kind === "point") {
      if (!value.target || Object.hasOwn(value, "tool") || Object.hasOwn(value, "role")) throw previewFault("invalid_preview", "Point previews require only an explicit semantic target");
    } else if (value.kind === "construction") {
      if (!["segment", "polyline", "center_radius_circle", "two_point_aligned_rectangle"].includes(value.tool)
        || !["profile", "construction"].includes(value.role ?? "profile") || Object.hasOwn(value, "target")) throw previewFault("invalid_preview", "Expected one supported construction tool and role");
    } else throw previewFault("invalid_preview", "Expected point or construction preview");
  } else {
    if (!["advance", "finish", "cancel"].includes(value.action) || typeof value.ticket !== "string" || !/^[a-f0-9]{64}$/u.test(value.ticket)) throw previewFault("invalid_preview", "Expected an opaque preview ticket");
    if (value.action === "advance" && (!Array.isArray(value.samples) || !value.samples.length || value.samples.length > 256)) throw previewFault("invalid_preview", "Preview batch must contain 1–256 ordered samples");
  }
  return value;
}

/** Host HTTP transport authenticates its session token first and supplies its
 * immutable connection. This adapter accepts no source, files or publication. */
export function createCollaborationPreviewRoute(service) {
  if (!service || typeof service.request !== "function") throw TypeError("Expected authoring preview service");
  return (connection, body, options) => service.request(connection, validatePreviewRequest(body), options);
}
