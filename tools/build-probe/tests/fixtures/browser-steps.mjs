import { join } from "node:path";
import { pathToFileURL } from "node:url";

// The browser driver comes from the package installation that SSR_PACKAGES names
// (tools/install_packages.py); the scripts run from a copy of the fixture sources, which has no
// node_modules, so the import names its absolute path.
const packages = process.env.SSR_PACKAGES;
if (!packages) {
  throw new Error("SSR_PACKAGES is not set; it names the package installation of tools/install_packages.py");
}
export const { chromium } = await import(
  pathToFileURL(join(packages, "node_modules/playwright-core/index.mjs")).href
);

// Page steps wait for page events and DOM markers. Their limit only detects a hung step: a
// step normally finishes within half a second, so the limit is about one hundred times that.
// The browser launch and close are long operations with no limit (`timeout: 0` for launch);
// their RUN and DONE lines report them, and their result or error decides them.
export const STEP_TIMEOUT = 60_000;

export function begin(name) {
  console.log(`RUN ${name}`);
}

export async function step(name, action) {
  begin(name);
  const started = performance.now();
  const result = await action();
  console.log(`DONE ${name} ${((performance.now() - started) / 1000).toFixed(3)}s`);
  return result;
}
