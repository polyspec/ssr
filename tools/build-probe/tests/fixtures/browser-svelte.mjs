import assert from "node:assert/strict";
import { chromium } from "playwright-core";

const [base, executablePath] = process.argv.slice(2);
assert.ok(base && executablePath);
const browser = await chromium.launch({ executablePath, headless: true });
try {
  const page = await browser.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  for (const [path, label, hydration, count] of [
    ["/ssr", "Ada", "yes", "5"],
    ["/csr", "Ada", "no", "4"],
    ["/shell/first", "/shell/first", "no", "0"],
    ["/shell/second", "/shell/second", "no", "0"],
  ]) {
    await page.goto(base + path, { waitUntil: "load", timeout: 5000 });
    await page.waitForSelector("#root button", { timeout: 5000 });
    assert.equal(await page.locator("#root h1").textContent(), label, path);
    assert.equal(await page.locator("#root button").textContent(), "0", path);
    assert.equal(await page.evaluate(() => window.__clientRenderCall), hydration === "yes" ? "hydrate" : "mount", path);
    assert.equal(await page.locator("#root main").evaluate((node) => getComputedStyle(node).color), "rgb(18, 52, 86)", path);
    if (hydration === "yes") {
      assert.equal(await page.locator('head meta[name="svelte-head"]').getAttribute("content"), "Ada", path);
      assert.equal(await page.evaluate(() => window.__svelteHead), true, path);
    }
    const state = await page.evaluate(() => JSON.parse(document.getElementById("__SSR_STATE__").textContent));
    if (path.startsWith("/shell/")) {
      assert.equal(state, null, path);
    } else {
      assert.deepEqual(state, { count: Number(count) }, path);
    }
    if (hydration === "yes") {
      assert.equal(await page.evaluate(() => window.__before === document.querySelector("#root main")), true, path);
    }
    await page.locator("#root button").click();
    assert.equal(await page.locator("#root button").textContent(), "1", path);
    assert.deepEqual(errors, [], `${path}: browser JavaScript errors`);
    console.log(`PASS ${path}`);
  }
  await page.goto(base + "/empty-ssr", { waitUntil: "load", timeout: 5000 });
  await page.waitForSelector("#root[data-render=ssr]", { state: "attached", timeout: 5000 });
  assert.equal(await page.locator("#root").evaluate((root) => root.children.length), 0);
  assert.equal(await page.evaluate(() => window.__clientRenderCall), "hydrate");
  assert.deepEqual(errors, [], "empty SSR: browser JavaScript errors");
  console.log("PASS /empty-ssr");
  const emptyBytes = await browser.newPage();
  try {
    const mismatch = emptyBytes.waitForEvent("pageerror", { timeout: 5000 });
    await emptyBytes.goto(base + "/empty-html-ssr", { waitUntil: "load", timeout: 5000 });
    assert.equal(await emptyBytes.locator("#root").evaluate((root) => root.innerHTML), "");
    assert.equal(await emptyBytes.evaluate(() => window.__clientRenderCall), "hydrate");
    assert.match((await mismatch).message, /hydration/i);
    console.log("PASS /empty-html-ssr");
  } finally {
    await emptyBytes.close();
  }
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
