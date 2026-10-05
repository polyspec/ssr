[Korean](requirements.ko.md)

# Requirements

The render, build and engine requirements of ssr (S-0-1).

- Separate Rust processes build and render; no Node process runs at build time or at render time.
  `npm ci` installs packages before a build.
- A render call crosses no foreign function boundary: the caller is Rust and calls the library
  directly.
- The bundle is loaded and compiled once per V8 isolate, not sent or compared on every call.
- Props enter the isolate once per call as a V8 value; the result leaves as bytes without a second
  JSON round trip.
- A context reset restores the global state from a snapshot instead of rebuilding it.
- ssr provides: SSR or CSR selected per render with one client bundle, the render state
  input and output for hydration, static CSR shells, publication of the public build files into a
  directory, serving of the build files, a bounded pool with a queue, a render timeout that
  terminates a running script, and operating system random values.
