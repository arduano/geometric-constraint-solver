#!/usr/bin/env node
// SPDX-License-Identifier: GPL-3.0-or-later
import { runCli } from "../runtime/geosolve-cli.mjs";

try {
  const result = await runCli(process.argv.slice(2));
  console.log(JSON.stringify(result, null, 2));
  if (result.ok === false) process.exitCode = 1;
} catch (error) {
  console.error(JSON.stringify({ ok: false, error: String(error), ...error.details }, null, 2));
  process.exitCode = 1;
}
