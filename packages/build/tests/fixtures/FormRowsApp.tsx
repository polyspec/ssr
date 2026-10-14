import React from "react";

// A form with a repeated row group that holds a repeated child group. Each row key is `row-` and
// eight hexadecimal digits created during the render from crypto.getRandomValues, so every render
// gives new keys. A control names its row by the key path `rows.<key>.<field>`.
function rowKey(): string {
  const bytes = globalThis.crypto.getRandomValues(new Uint8Array(4));
  return `row-${Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}

export default function App() {
  const parent = rowKey();
  const child = rowKey();
  return (
    <form>
      <input name={`rows.${parent}.title`} defaultValue="" />
      <input name={`rows.${parent}.children.${child}.title`} defaultValue="" />
    </form>
  );
}
