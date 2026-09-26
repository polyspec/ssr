import React from "react";
import { renderToString } from "react-dom/server.edge";

(globalThis as any).__ssrReact = React;
(globalThis as any).renderBridge = (App: React.ComponentType<{ name: string }>) =>
  renderToString(React.createElement(App, { name: "Ada" }));
