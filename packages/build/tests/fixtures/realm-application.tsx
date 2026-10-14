import React from "react";

const Label = React.createContext("missing");

class Boundary extends React.Component<{ children: React.ReactNode }> {
  render() {
    return this.props.children;
  }
}

function App({ name }: { name: string }) {
  if (typeof setTimeout !== "undefined") {
    throw new Error("application timer is visible");
  }
  const id = React.useId();
  return React.createElement(
    Label.Provider,
    { value: name },
    React.createElement(Boundary, null, React.createElement(Heading, { id })),
  );
}

function Heading({ id }: { id: string }) {
  return React.createElement("h1", { id }, React.useContext(Label));
}

(globalThis as any).AppBridge = App;
(globalThis as any).__ssrApp = App;
