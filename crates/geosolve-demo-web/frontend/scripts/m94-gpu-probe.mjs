// SPDX-License-Identifier: GPL-3.0-or-later
import { chromium } from '@playwright/test';
const configs = [[], ['--use-gl=angle','--use-angle=gl','--enable-gpu','--ignore-gpu-blocklist'], ['--use-gl=angle','--use-angle=vulkan','--enable-features=Vulkan','--disable-vulkan-surface','--enable-gpu','--ignore-gpu-blocklist']];
for (const args of configs) {
 let browser;
 try {
  browser = await chromium.launch({executablePath:'/home/arduano/.nix-profile/bin/google-chrome', args, timeout:15000});
  const page = await browser.newPage();
  const result = await page.evaluate(() => {
    const canvas = document.createElement('canvas'); const gl=canvas.getContext('webgl2');
    if(!gl) return null; const debug=gl.getExtension('WEBGL_debug_renderer_info');
    return {renderer:gl.getParameter(debug?.UNMASKED_RENDERER_WEBGL??gl.RENDERER), vendor:gl.getParameter(debug?.UNMASKED_VENDOR_WEBGL??gl.VENDOR),version:gl.getParameter(gl.VERSION)};
  });
  console.log(JSON.stringify({args,result}));
 } catch (error) { console.log(JSON.stringify({args,error:String(error)})); }
 finally { await browser?.close(); }
}
