// SPDX-License-Identifier: GPL-3.0-or-later
import { LoaderCircle } from "lucide-react";

export function SolvingIndicator() {
  return <div className="geosolve-solving-overlay">
    <div className="geosolve-solving-message" role="status" aria-live="polite" aria-atomic="true">
      <LoaderCircle className="geosolve-solving-spinner" aria-hidden="true" />
      <div><div className="font-medium text-foreground">Solving…</div><div className="mt-1 text-xs text-muted">Your last accepted sketch stays visible.</div></div>
    </div>
  </div>;
}
