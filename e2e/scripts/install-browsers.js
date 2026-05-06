#!/usr/bin/env node
// Symlinks system Chromium to where Playwright expects its headless shell.
// Needed on distros not yet supported by Playwright's bundled browser downloads.

const fs = require('fs');
const path = require('path');
const os = require('os');

const SYSTEM_CHROMIUM = [
  '/usr/bin/chromium-browser',
  '/usr/bin/chromium',
  '/snap/bin/chromium',
].find(p => fs.existsSync(p));

if (!SYSTEM_CHROMIUM) {
  console.error('System Chromium not found. Install with: sudo apt install chromium-browser');
  process.exit(1);
}

const browsersJsonPath = require.resolve('playwright-core').replace(/index\.js$/, 'browsers.json');
const browsersJson = JSON.parse(require('fs').readFileSync(browsersJsonPath, 'utf8'));
const entry = browsersJson.browsers.find(b => b.name === 'chromium-headless-shell');
if (!entry) {
  console.error('chromium-headless-shell not found in playwright-core/browsers.json');
  process.exit(1);
}

const browsersBase = process.env.PLAYWRIGHT_BROWSERS_PATH
  ?? path.join(os.homedir(), '.cache', 'ms-playwright');
const dir = path.join(browsersBase, `chromium_headless_shell-${entry.revision}`, 'chrome-headless-shell-linux64');
const target = path.join(dir, 'chrome-headless-shell');

fs.mkdirSync(dir, { recursive: true });
if (fs.existsSync(target)) fs.unlinkSync(target);
fs.symlinkSync(SYSTEM_CHROMIUM, target);

console.log(`Linked ${SYSTEM_CHROMIUM} → ${target}`);
