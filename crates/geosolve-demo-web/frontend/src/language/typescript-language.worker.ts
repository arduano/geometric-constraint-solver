// SPDX-License-Identifier: GPL-3.0-or-later

/// <reference lib="webworker" />

import {
  handleTypeScriptLanguageRequest,
  TypeScriptProjectLanguageService,
} from "./language-service";
import type { TypeScriptLanguageRequest } from "./protocol";

const languageService = new TypeScriptProjectLanguageService();

self.addEventListener("message", (event: MessageEvent<TypeScriptLanguageRequest>) => {
  const response = handleTypeScriptLanguageRequest(languageService, event.data);
  if (response) self.postMessage(response);
});
