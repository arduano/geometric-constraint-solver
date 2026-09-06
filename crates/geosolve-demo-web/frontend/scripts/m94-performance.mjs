// SPDX-License-Identifier: GPL-3.0-or-later
// Separate viewport submission CPU, browser paint/raster, and end-to-end camera latency.
import { chromium, expect } from '@playwright/test';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { resolve, dirname } from 'node:path';
const root = resolve(import.meta.dirname, '../../../..');
const url = process.env.GEOSOLVE_E2E_BASE_URL ?? 'http://127.0.0.1:18094/';
const output = resolve(process.env.M94_PERFORMANCE_OUTPUT ?? resolve(root, 'target/m94/performance.json'));
const args = process.env.M94_HARDWARE === '1' ? ['--use-gl=angle', '--use-angle=vulkan', '--enable-features=Vulkan', '--disable-vulkan-surface', '--enable-gpu', '--ignore-gpu-blocklist'] : [];
const browser = await chromium.launch({ executablePath: process.env.GEOSOLVE_CHROMIUM_PATH ?? '/home/arduano/.nix-profile/bin/google-chrome', args });
const catalog = JSON.parse(await readFile(resolve(root, 'crates/geosolve-sketch-code/assets/bundled-sample-catalog.json'), 'utf8'));
const samples = ['theo-jansen-leg', 'pc-water-manifold', 'perforated-fixture-field', 'robotic-harness-backplane'];
const system = await (await browser.newBrowserCDPSession()).send('SystemInfo.getInfo');
const report = { format: 'geosolve-m94-camera-performance-v1', url, args, browser: browser.version(), system, capturedAt: new Date().toISOString(), samples: [] };
const percentile = (values, fraction) => [...values].sort((a,b) => a-b)[Math.min(values.length-1, Math.floor(values.length*fraction))];
try {
 for (const key of samples) {
  const sample = catalog.samples.find((sample) => sample.key === key);
  const context = await browser.newContext({viewport:{width:1440,height:900}});
  const page = await context.newPage(); page.setDefaultTimeout(90000);
  await page.goto(url,{waitUntil:'networkidle'});
  await page.getByRole('button',{name:'File menu'}).click(); await page.getByRole('menuitem',{name:/Open/}).click();
  await page.getByPlaceholder(`Search ${catalog.samples.length} samples…`).fill(key);
  await page.getByRole('dialog',{name:'Open project'}).getByRole('button').filter({has:page.getByText(sample.title,{exact:true})}).click();
  await page.getByRole('button',{name:'design',exact:true}).click(); await page.getByRole('button',{name:'Fit sketch'}).click();
  const canvas = page.locator('[role="application"] canvas'); const isCanvas = await canvas.count() === 1;
  if (isCanvas) await expect(canvas).toHaveAttribute('data-render-state','ready');
  const host = page.getByRole('application'); const box=await host.boundingBox();
  await page.mouse.move(box.x+box.width*.6,box.y+box.height*.6);
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  const session = await context.newCDPSession(page); const traces=[];
  session.on('Tracing.dataCollected', event => traces.push(...event.value));
  await session.send('Tracing.start',{categories:'devtools.timeline,disabled-by-default-devtools.timeline',transferMode:'ReportEvents'});
  const latencies=[], submissions=[];
  for(let i=0;i<24;i++) {
   await page.evaluate(() => {globalThis.__m94InputStart=performance.now();});
   await page.mouse.wheel(0,i%2?-12:12);
   latencies.push(await page.evaluate(() => new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(()=>resolve(performance.now()-globalThis.__m94InputStart))))));
   if(isCanvas) submissions.push(await canvas.evaluate(element=>element.__geosolveRendererDiagnostics.lastRenderMilliseconds));
  }
  const completion=new Promise(resolve=>session.once('Tracing.tracingComplete',resolve)); await session.send('Tracing.end'); await completion;
  const relevant=traces.filter(event=>event.ph==='X'&&['Paint','RasterTask','UpdateLayerTree'].includes(event.name));
  const diagnostics=isCanvas?await canvas.evaluate(element=>element.__geosolveRendererDiagnostics):null;
  const paints={}; for(const event of relevant) paints[event.name]=(paints[event.name]??0)+(event.dur??0)/1000;
  report.samples.push({key,renderer:isCanvas?'webgl2':'svg',latencyMilliseconds:latencies,p50:percentile(latencies,.5),p95:percentile(latencies,.95),submissionMilliseconds:submissions,submissionP50:submissions.length?percentile(submissions,.5):null,submissionP95:submissions.length?percentile(submissions,.95):null,browserPaintTaskMilliseconds:paints,diagnostics});
  await mkdir(dirname(output),{recursive:true}); await writeFile(output,JSON.stringify(report,null,2));
  console.log(`${key}: end-to-end p50 ${percentile(latencies,.5).toFixed(1)} ms, p95 ${percentile(latencies,.95).toFixed(1)} ms`);
  await context.close();
 }
} finally {await browser.close();}
