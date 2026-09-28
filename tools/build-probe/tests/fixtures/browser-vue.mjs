import assert from "node:assert/strict";
import { chromium } from "playwright-core";

const [base, executablePath, scenario] = process.argv.slice(2);
assert.ok(base && executablePath);
assert.ok(scenario === "main" || scenario === "empty", "known browser scenario required");
const browser = await chromium.launch({ executablePath, headless: true });
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
      await page.goto(base + path, { waitUntil: "load", timeout: 5000 });
      await page.waitForSelector("#root button", { timeout: 5000 });
      assert.equal(await page.locator("#root h1").textContent(), label, path);
      assert.equal(await page.locator("#root button").textContent(), "0", path);
      assert.equal(await page.evaluate(() => window.__clientRenderCall), hydration === "yes" ? "createSSRApp" : "createApp", path);
      assert.equal(await page.evaluate(() => JSON.parse(document.getElementById("__SSR_STATE__").textContent)?.count ?? 0), Number(count), path);
      if (hydration === "yes") {
        assert.equal(await page.evaluate(() => window.__before === document.querySelector("#root main")), true, path);
      }
      await page.locator("#root button").click();
      assert.equal(await page.locator("#root button").textContent(), "1", path);
      assert.deepEqual(errors, [], `${path}: browser JavaScript errors`);
      console.log(`PASS ${path}`);
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
  } else {
    await page.goto(base + "/empty-ssr", { waitUntil: "load", timeout: 5000 });
    await page.waitForSelector("#root[data-render=ssr]", { state: "attached", timeout: 5000 });
    assert.equal(await page.locator("#root").evaluate((root) => root.children.length), 0);
    assert.equal(await page.evaluate(() => window.__clientRenderCall), "createSSRApp");
    assert.deepEqual(errors, [], "empty SSR: browser JavaScript errors");
    console.log("PASS /empty-ssr");
  }
} finally {
  await browser.close();
}
