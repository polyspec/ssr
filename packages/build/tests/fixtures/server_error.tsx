function fail(): never {
  throw new Error("mapped render failure");
}

export function render() {
  return fail();
}
