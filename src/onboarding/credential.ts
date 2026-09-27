/**
 * Credential handling for the OpenRouter key (docs/threat-model.md, T12).
 *
 * THE THREAT. The provider key is the authority to spend money and read data.
 * The agent runs as the same user as this app. So a key that reaches a log file,
 * a protocol message, an error string, or the webview is a key **the agent can
 * read and use directly** — with no gate, no approval, no card. The product's
 * central promise becomes optional the moment its credential is reachable.
 *
 * THE APPROACH HERE IS STRUCTURAL, NOT DISCIPLINARY. Telling people "never log
 * the key" is a rule that gets broken by the next person in a hurry. Instead the
 * key is wrapped in an object that **cannot be serialised or stringified into
 * anything but `[redacted]`**, so the leak has to be deliberate — you have to call
 * `reveal()`, which is one greppable word in a code review.
 *
 * WHAT THIS DOES NOT DO, stated plainly. It does not encrypt anything, and it
 * does not stop code that already has the `Secret` from calling `reveal()`. The
 * operating system does that part — see the storage note at the bottom.
 */

/**
 * A secret that refuses to be turned into text accidentally.
 *
 * `JSON.stringify`, template literals, and `console.log` all go through the
 * methods below, so a secret logged by accident comes out as `[redacted]` rather
 * than as the key.
 */
export class Secret {
  readonly #value: string;

  constructor(value: string) {
    if (typeof value !== 'string' || value.length === 0) {
      throw new Error('a secret cannot be empty');
    }
    this.#value = value;
  }

  /**
   * Get the real value. The ONLY way out, and deliberately a distinctive word so
   * that `reveal()` in a code review is a question someone has to answer.
   */
  reveal(): string {
    return this.#value;
  }

  /** `JSON.stringify` uses this, so serialising a secret cannot leak it. */
  toJSON(): string {
    return '[redacted]';
  }

  /** Template literals and `console.log` use this. */
  toString(): string {
    return '[redacted]';
  }

  /** `util.inspect`, and therefore most logger output. */
  [Symbol.for('nodejs.util.inspect.custom')](): string {
    return '[redacted]';
  }

  /** Length only, for diagnostics that need to say something without saying it. */
  get length(): number {
    return this.#value.length;
  }

  /** Compare without revealing. Useful for "is this the same key we stored?". */
  equals(other: Secret): boolean {
    return this.#value === other.#value;
  }
}

/** Anything that can hold a credential. Implemented by the OS store in Rust. */
export interface CredentialStore {
  /** Store or replace a secret. */
  save(name: string, secret: Secret): Promise<void>;
  /** Retrieve it, or undefined when nothing is stored under that name. */
  load(name: string): Promise<Secret | undefined>;
  /** Remove it. Removing something absent is not an error. */
  delete(name: string): Promise<void>;
}

/**
 * An in-memory store, for tests and for a machine with no OS store available.
 *
 * Deliberately NOT persisted anywhere. A file-backed fallback would be exactly
 * the `token.json` mistake this whole module exists to prevent, so there is no
 * "convenient" implementation to reach for by accident.
 */
export class InMemoryCredentialStore implements CredentialStore {
  readonly #entries = new Map<string, Secret>();

  async save(name: string, secret: Secret): Promise<void> {
    this.#entries.set(name, secret);
  }

  async load(name: string): Promise<Secret | undefined> {
    return this.#entries.get(name);
  }

  async delete(name: string): Promise<void> {
    this.#entries.delete(name);
  }

  /** Test helper: how many credentials are held. Never returns the values. */
  size(): number {
    return this.#entries.size;
  }
}

/**
 * Remove a secret from a string, for the places where text is built from
 * user-visible data and might accidentally include one.
 *
 * Belt and braces beside `Secret`: this catches the case where a key has already
 * been unwrapped into a plain string and then embedded in a message.
 */
export function redact(text: string, ...secrets: Array<Secret | string>): string {
  let result = text;
  for (const candidate of secrets) {
    const value = typeof candidate === 'string' ? candidate : candidate.reveal();
    if (value.length === 0) continue;
    result = result.split(value).join('[redacted]');
  }
  return result;
}

/**
 * STORAGE, AND THE ONE PIECE STILL MISSING.
 *
 * The `CredentialStore` above is the interface. The implementation that matters
 * is **not in this file and not in TypeScript**, because it must be the Rust shell
 * that talks to the Windows Credential Manager — not the sidecar, which is the
 * component that runs alongside untrusted content, and not the webview, which is
 * the component an XSS would reach.
 *
 * Until that exists, `InMemoryCredentialStore` is the only implementation, and it
 * forgets everything on exit. **That is the intended failure direction**: no
 * storage means not connected, never a key written somewhere convenient.
 */
