(() => {
  const encodeUtf8 = globalThis.__ssrEncodeUtf8;
  delete globalThis.__ssrEncodeUtf8;

  function domString(value) {
    if (typeof value === "symbol") throw new TypeError("TextEncoder input cannot be a Symbol");
    return String(value);
  }

  globalThis.TextEncoder = class TextEncoder {
    #brand = true;

    // The brand check of WebIDL: a method or getter called on another receiver throws a TypeError, since reading the
    // private field of an object that lacks it throws one.
    static #check(receiver) {
      if (receiver.#brand !== true) throw new TypeError("Illegal invocation");
    }

    get encoding() {
      TextEncoder.#check(this);
      return "utf-8";
    }

    encode(input = "") {
      TextEncoder.#check(this);
      return encodeUtf8(domString(input));
    }

    encodeInto(source, destination) {
      TextEncoder.#check(this);
      if (arguments.length < 2) throw new TypeError("encodeInto requires source and destination");
      source = domString(source);
      if (!(destination instanceof Uint8Array)) {
        throw new TypeError("encodeInto destination must be a Uint8Array");
      }
      let read = 0;
      let written = 0;
      while (read < source.length) {
        let point = source.codePointAt(read);
        const units = point > 0xffff ? 2 : 1;
        if (point >= 0xd800 && point <= 0xdfff) point = 0xfffd;
        const length = point <= 0x7f ? 1 : point <= 0x7ff ? 2 : point <= 0xffff ? 3 : 4;
        if (written + length > destination.length) break;
        if (length === 1) {
          destination[written] = point;
        } else if (length === 2) {
          destination[written] = 0xc0 | (point >> 6);
          destination[written + 1] = 0x80 | (point & 0x3f);
        } else if (length === 3) {
          destination[written] = 0xe0 | (point >> 12);
          destination[written + 1] = 0x80 | ((point >> 6) & 0x3f);
          destination[written + 2] = 0x80 | (point & 0x3f);
        } else {
          destination[written] = 0xf0 | (point >> 18);
          destination[written + 1] = 0x80 | ((point >> 12) & 0x3f);
          destination[written + 2] = 0x80 | ((point >> 6) & 0x3f);
          destination[written + 3] = 0x80 | (point & 0x3f);
        }
        read += units;
        written += length;
      }
      return { read, written };
    }
  };
})();
