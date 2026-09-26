import assert from "node:assert/strict";
import { chromium } from "playwright-core";

const [base, executablePath] = process.argv.slice(2);
assert.ok(base && executablePath, "browser test requires the server URL and browser path");

const browser = await chromium.launch({ executablePath, headless: true });
try {
  const page = await browser.newPage();
  await page.goto(base + "/recover", { waitUntil: "load", timeout: 5000 });
  await page.waitForSelector("#recovered", { timeout: 5000 });
  assert.equal(await page.locator("#recovered").textContent(), "Recovered");
  assert.equal(await page.locator("#fallback").count(), 0);
  console.log("PASS /recover");

  await page.goto(base + "/client-error", { waitUntil: "load", timeout: 5000 });
  await page.waitForSelector("#client-error", { timeout: 5000 });
  assert.equal(await page.locator("#client-error").textContent(), "Client error");
  console.log("PASS /client-error");
} finally {
  await browser.close();
}
