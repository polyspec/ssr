import React, { useEffect } from "react";

export default function App({ name, empty, renderState }: { name?: string; empty?: boolean; renderState: { input: { count?: number } | null; output: unknown } }) {
  const label = name ?? window.location.pathname;
  renderState.output = { count: (renderState.input?.count ?? 0) + 1 };
  useEffect(() => {
    document.documentElement.dataset.hydrated =
      (window as any).__before === document.querySelector("#root main") ? "yes" : "no";
    document.documentElement.dataset.inputCount = String(renderState.input?.count ?? 0);
  }, []);
  return empty ? null : <main><h1>{label}</h1></main>;
}
