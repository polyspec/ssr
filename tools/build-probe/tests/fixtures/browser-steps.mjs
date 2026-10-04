// Browser steps wait for page events and DOM markers. Their time limits only detect a hung
// step: a step normally finishes within half a second and a browser launch within a few
// seconds, so the limits are about one hundred times those durations.
export const STEP_TIMEOUT = 60_000;
export const LAUNCH_TIMEOUT = 300_000;

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
