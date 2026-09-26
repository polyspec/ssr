import React from "react";
import * as JSX from "react/jsx-runtime";
import { renderToReadableStream } from "react-dom/server.edge";
import { ReadableStream as SSRReadableStream } from "web-streams-polyfill";

ReadableStream = SSRReadableStream;
(globalThis as any).__ssrJsxRuntime = JSX;
(globalThis as any).__ssrReact = React;
(globalThis as any).renderBridge = (App: React.ComponentType<{ name: string }>) =>
  renderToReadableStream(React.createElement(App, { name: "Ada" }), { nonce: "sample" });
(globalThis as any).render = async (App: React.ComponentType<{ name: string }>, props: { name: string }, state: unknown, nonce: string) => {
  const stream = await renderToReadableStream(React.createElement(App, props), {
    nonce,
    onError: (globalThis as any).__ssrReportReactError,
  });
  return { stream, state };
};
