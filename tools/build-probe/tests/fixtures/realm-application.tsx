import React from "react";

function App({ name }: { name: string }) {
  if (typeof setTimeout !== "undefined") {
    throw new Error("application timer is visible");
  }
  return React.createElement("h1", null, name);
}

(globalThis as any).AppBridge = App;
