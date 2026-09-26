import assert from "node:assert/strict";
import { chromium } from "playwright-core";

const [base, executablePath] = process.argv.slice(2);
assert.ok(base && executablePath, "browser test requires the server URL and browser path");

const browser = await chromium.launch({ executablePath, headless: true });
try {
  const page = await browser.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  for (const [path, expected, hydration, count] of [
    ["/ssr", "Ada", "yes", "5"],
    ["/csr", "Ada", "no", "4"],
    ["/escape", "</script><script>window.attack=1</script>", "no", "4"],
    ["/shell/first", "/shell/first", "no", "0"],
    ["/shell/second", "/shell/second", "no", "0"],
  ]) {
    await page.goto(base + path, { waitUntil: "load", timeout: 5000 });
    await page.waitForSelector("html[data-input-count]", { timeout: 5000 });
    assert.equal(await page.locator("#root h1").textContent(), expected, path);
    assert.equal(await page.locator("html").getAttribute("data-hydrated"), hydration, path);
    assert.equal(await page.locator("html").getAttribute("data-input-count"), count, path);
    assert.equal(await page.evaluate(() => window.__clientRenderCall), hydration === "yes" ? "hydrateRoot" : "createRoot", path);
    assert.equal(await page.evaluate(() => globalThis.attack), undefined, path);
    assert.deepEqual(errors, [], `${path}: browser JavaScript errors`);
    console.log(`PASS ${path}`);
  }
  await page.goto(base + "/empty-ssr", { waitUntil: "load", timeout: 5000 });
  await page.waitForSelector("html[data-input-count]", { timeout: 5000 });
  assert.equal(await page.locator("#root").getAttribute("data-render"), "ssr");
  assert.equal(await page.locator("#root").evaluate((root) => root.children.length), 0);
  assert.equal(await page.evaluate(() => window.__clientRenderCall), "hydrateRoot");
  assert.deepEqual(errors, [], "empty SSR: browser JavaScript errors");
  console.log("PASS /empty-ssr");
  for (const path of ["/missing-mode", "/invalid-mode"]) {
    const invalid = await browser.newPage();
    try {
      const failure = invalid.waitForEvent("pageerror", { timeout: 5000 });
      await invalid.goto(base + path, { waitUntil: "load", timeout: 5000 });
      assert.match((await failure).message, /Invalid render mode/, path);
      assert.equal(await invalid.evaluate(() => window.__clientRenderCall), undefined, path);
      console.log(`PASS ${path}`);
    } finally {
      await invalid.close();
    }
  }
} finally {
  await browser.close();
}
