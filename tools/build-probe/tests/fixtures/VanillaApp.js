function label(props) {
  return props.name ?? location.pathname;
}

export function render(props, renderState) {
  renderState.output = { count: renderState.input.count + 1 };
  return `<main><h1>${props.name}</h1><button>0</button></main>`;
}

function connect(root, props, renderState) {
  const main = root.querySelector("main");
  const button = main.querySelector("button");
  button.addEventListener("click", () => {
    button.textContent = String(Number(button.textContent) + 1);
  });
  document.documentElement.dataset.inputCount = String(renderState.input?.count ?? 0);
  document.documentElement.dataset.name = label(props);
  return main;
}

export function hydrate(root, props, renderState) {
  if (!root.querySelector("main")) throw new Error("server DOM missing");
  document.documentElement.dataset.hydrated = "yes";
  connect(root, props, renderState);
}

export function mount(root, props, renderState) {
  const main = document.createElement("main");
  const heading = document.createElement("h1");
  heading.textContent = label(props);
  const button = document.createElement("button");
  button.textContent = "0";
  main.append(heading, button);
  root.append(main);
  document.documentElement.dataset.hydrated = "no";
  connect(root, props, renderState);
}
