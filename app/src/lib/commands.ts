// The one typed seam in front of every Tauri command (Candidate B). Import `cmd` here instead of
// calling `invoke("…")` with a hand-encoded name + argument bag: the names, argument order, and
// return/error types all come from `bindings.ts`, which tauri-specta regenerates from the Rust
// command signatures (see src-tauri/src/lib.rs). A backend rename or signature change becomes a
// compile error at the call site rather than a silent runtime failure.
//
// `bindings.ts` models a `Result<T, E>` command as a `{ status: "ok"; data } | { status: "error"; error }`
// union. `cmd` unwraps that back to the throw-on-error shape the call sites already expect from raw
// `invoke` — on success it resolves with `data`; on failure it throws the exact error value the
// backend returned (a string, or the typed `{ code, message }` AppError), identical to what `invoke`
// rejected with before. Commands that return a plain value (no Rust `Result`, e.g. `read_git_log`)
// are passed straight through.
import { commands as __commands } from "./bindings";

type Result<T, E> = { status: "ok"; data: T } | { status: "error"; error: E };

// Map each generated command from its Result-returning shape to the unwrapped, throw-on-error shape.
// Non-Result commands (plain value returns) keep their signature unchanged.
type Unwrap<F> = F extends (...args: infer A) => Promise<Result<infer T, unknown>>
  ? (...args: A) => Promise<T>
  : F;
type Cmd = { [K in keyof typeof __commands]: Unwrap<(typeof __commands)[K]> };

function isResult(v: unknown): v is Result<unknown, unknown> {
  return (
    typeof v === "object" &&
    v !== null &&
    "status" in v &&
    ((v as { status: unknown }).status === "ok" || (v as { status: unknown }).status === "error")
  );
}

function unwrap(value: unknown): unknown {
  if (!isResult(value)) return value; // plain-value command (no Rust Result) — pass straight through
  if (value.status === "error") throw value.error;
  return value.data;
}

export const cmd: Cmd = new Proxy({} as Cmd, {
  get(_target, key: string) {
    const fn = (__commands as Record<string, unknown>)[key];
    if (typeof fn !== "function") return fn;
    return (...args: unknown[]) => (fn as (...a: unknown[]) => Promise<unknown>)(...args).then(unwrap);
  },
});
