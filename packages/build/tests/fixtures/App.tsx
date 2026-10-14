import React from "react";
import png from "./assets/sample.png";
import svg from "./assets/sample.svg";
import jpg from "./assets/sample.jpg";
import gif from "./assets/sample.gif";
import webp from "./assets/sample.webp";
import avif from "./assets/sample.avif";
import ico from "./assets/sample.ico";
import woff from "./assets/sample.woff";
import woff2 from "./assets/sample.woff2";
import ttf from "./assets/sample.ttf";

export const assets = [png, svg, jpg, gif, webp, avif, ico, woff, woff2, ttf];

export function App() {
  return <main><h1>Build verification</h1><img src={svg} alt="sample" /></main>;
}
