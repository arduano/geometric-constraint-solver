// SPDX-License-Identifier: GPL-3.0-or-later

import { compileManagedSource } from "../lib/managed-compiler";

Reflect.set(globalThis, "__geosolveCompileManagedSource", compileManagedSource);
document.body.dataset.compilerReady = "true";
