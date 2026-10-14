import React from "react";
import { createRoot } from "react-dom/client";
import { App, assets } from "./App";

export function mount(element: Element) {
  createRoot(element).render(<App />);
  return assets;
}

export async function loadExtra() {
  return import("./extra");
}
