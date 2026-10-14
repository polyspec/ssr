import assert from "node:assert/strict";
import { STEP_TIMEOUT, begin, chromium, step } from "./browser-steps.mjs";

const [base, executablePath, scenario] = process.argv.slice(2);
assert.ok(base && executablePath);
assert.ok(scenario === "main" || scenario === "empty", "known browser scenario required");
const browser = await step("launch", () => chromium.launch({ executablePath, headless: true, timeout: 0 }));
try {
  const page = await browser.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  if (scenario === "main") {
    for (const [path, label, hydration, count] of [
      ["/ssr", "Ada", "yes", "5"],
      ["/csr", "Ada", "no", "4"],
      ["/shell/first", "/shell/first", "no", "0"],
      ["/shell/second", "/shell/second", "no", "0"],
    ]) {
      begin(path);
      await page.goto(base + path, { waitUntil: "load", timeout: STEP_TIMEOUT });
      await page.waitForSelector("#root button", { timeout: STEP_TIMEOUT });
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
    for (const path of ["/missing-mode", "/invalid-mode"]) {
      begin(path);
      const invalid = await browser.newPage();
      try {
        const failure = invalid.waitForEvent("pageerror", { timeout: STEP_TIMEOUT });
        await invalid.goto(base + path, { waitUntil: "load", timeout: STEP_TIMEOUT });
        assert.match((await failure).message, /Invalid render mode/, path);
        assert.equal(await invalid.evaluate(() => window.__clientRenderCall), undefined, path);
        console.log(`PASS ${path}`);
      } finally {
        await invalid.close();
      }
    }
  } else {
    begin("/empty-ssr");
    await page.goto(base + "/empty-ssr", { waitUntil: "load", timeout: STEP_TIMEOUT });
    await page.waitForSelector("#root[data-render=ssr]", { state: "attached", timeout: STEP_TIMEOUT });
    assert.equal(await page.locator("#root").evaluate((root) => root.children.length), 0);
    assert.equal(await page.evaluate(() => window.__clientRenderCall), "hydrate");
    assert.deepEqual(errors, [], "empty SSR: browser JavaScript errors");
    console.log("PASS /empty-ssr");
    const emptyBytes = await browser.newPage();
    try {
      const mismatch = emptyBytes.waitForEvent("pageerror", { timeout: STEP_TIMEOUT });
      begin("/empty-html-ssr");
      await emptyBytes.goto(base + "/empty-html-ssr", { waitUntil: "load", timeout: STEP_TIMEOUT });
      assert.equal(await emptyBytes.locator("#root").evaluate((root) => root.innerHTML), "");
      assert.equal(await emptyBytes.evaluate(() => window.__clientRenderCall), "hydrate");
      assert.match((await mismatch).message, /hydration/i);
      console.log("PASS /empty-html-ssr");
    } finally {
      await emptyBytes.close();
    }
  }
} finally {
  await step("close", () => browser.close());
}
