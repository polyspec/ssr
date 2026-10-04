import assert from "node:assert/strict";
import { chromium } from "playwright-core";
import { LAUNCH_TIMEOUT, STEP_TIMEOUT, begin, step } from "./browser-steps.mjs";

const [base, executablePath] = process.argv.slice(2);
assert.ok(base && executablePath, "browser test requires the server URL and browser path");

const browser = await step("launch", () => chromium.launch({ executablePath, headless: true, timeout: LAUNCH_TIMEOUT }));
try {
  const page = await browser.newPage();
  begin("/recover");
  await page.goto(base + "/recover", { waitUntil: "load", timeout: STEP_TIMEOUT });
  await page.waitForSelector("#recovered", { timeout: STEP_TIMEOUT });
  assert.equal(await page.locator("#recovered").textContent(), "Recovered");
  assert.equal(await page.locator("#fallback").count(), 0);
  console.log("PASS /recover");

  begin("/client-error");
  await page.goto(base + "/client-error", { waitUntil: "load", timeout: STEP_TIMEOUT });
  await page.waitForSelector("#client-error", { timeout: STEP_TIMEOUT });
  assert.equal(await page.locator("#client-error").textContent(), "Client error");
  console.log("PASS /client-error");
} finally {
  await step("close", () => browser.close());
}
