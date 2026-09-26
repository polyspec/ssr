import React from "react";

function App({ name }: { name: string }) {
  if (typeof setTimeout !== "undefined") {
    throw new Error("application timer is visible");
  }
  const id = React.useId();
  return React.createElement("h1", { id }, name);
}

(globalThis as any).AppBridge = App;
