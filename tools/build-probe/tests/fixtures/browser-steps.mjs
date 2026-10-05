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
