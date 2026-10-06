(() => {
  const codes = new Map([
    ["TypeMismatchError", 17],
    ["QuotaExceededError", 22],
  ]);
  class DOMException extends Error {
    constructor(message = "", name = "Error") {
      super(String(message));
      this.name = String(name);
    }
    get code() { return codes.get(this.name) ?? 0; }
  }
  Object.defineProperty(DOMException.prototype, Symbol.toStringTag, { value: "DOMException" });
  Object.defineProperty(globalThis, "DOMException", { value: DOMException, configurable: true });
  return DOMException;
})();
