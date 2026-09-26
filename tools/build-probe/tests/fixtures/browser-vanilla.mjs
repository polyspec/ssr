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
    await page.waitForSelector("html[data-input-count]", { timeout: 5000 });
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
} finally {
  await browser.close();
}
