import React from "react";
import { renderToString } from "react-dom/server";
import { App, assets } from "./App";

export function render() {
  return renderToString(<App />) + assets.join("");
}

export async function loadExtra() {
  return import("./extra");
}
