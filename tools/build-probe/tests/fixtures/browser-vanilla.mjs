import assert from "node:assert/strict";
import { STEP_TIMEOUT, begin, chromium, step } from "./browser-steps.mjs";

const [base, executablePath] = process.argv.slice(2);
assert.ok(base && executablePath);
const browser = await step("launch", () => chromium.launch({ executablePath, headless: true, timeout: 0 }));
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
    begin(path);
    await page.goto(base + path, { waitUntil: "load", timeout: STEP_TIMEOUT });
    await page.waitForSelector("html[data-input-count]", { timeout: STEP_TIMEOUT });
    assert.equal(await page.locator("#root h1").textContent(), label, path);
    assert.equal(await page.locator("html").getAttribute("data-hydrated"), hydration, path);
    assert.equal(await page.locator("html").getAttribute("data-input-count"), count, path);
    if (hydration === "yes") {
      assert.equal(await page.evaluate(() => window.__before === document.querySelector("#root main")), true, path);
    }
    await page.locator("#root button").click();
    assert.equal(await page.locator("#root button").textContent(), "1", path);
    assert.deepEqual(errors, [], `${path}: browser JavaScript errors`);
    console.log(`PASS ${path}`);
  }
  begin("/empty-ssr");
  await page.goto(base + "/empty-ssr", { waitUntil: "load", timeout: STEP_TIMEOUT });
  await page.waitForSelector("html[data-input-count]", { timeout: STEP_TIMEOUT });
  assert.equal(await page.locator("#root").getAttribute("data-render"), "ssr");
  assert.equal(await page.locator("html").getAttribute("data-hydrated"), "yes");
  assert.equal(await page.locator("#root").evaluate((root) => root.childNodes.length), 0);
  assert.deepEqual(errors, [], "empty SSR: browser JavaScript errors");
  console.log("PASS /empty-ssr");
  for (const path of ["/missing-mode", "/invalid-mode"]) {
    begin(path);
    const invalid = await browser.newPage();
    try {
      const failure = invalid.waitForEvent("pageerror", { timeout: STEP_TIMEOUT });
      await invalid.goto(base + path, { waitUntil: "load", timeout: STEP_TIMEOUT });
      assert.match((await failure).message, /Invalid render mode/, path);
      console.log(`PASS ${path}`);
    } finally {
      await invalid.close();
    }
  }
} finally {
  await step("close", () => browser.close());
}
