function fail(): never {
  throw new Error("mapped render failure");
}

globalThis.render = () => fail();
