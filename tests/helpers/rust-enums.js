import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const configRs = readFileSync(
    fileURLToPath(new URL("../../src-tauri/core/src/config.rs", import.meta.url)),
    "utf8",
);

const toCamel = (name) => name[0].toLowerCase() + name.slice(1);
const toKebab = (name) => name.replace(/([a-z0-9])([A-Z])/g, "$1-$2").toLowerCase();
const RENAMES = { camelCase: toCamel, "kebab-case": toKebab, lowercase: (n) => n.toLowerCase() };

/**
 * The JSON strings serde accepts for a unit-variant enum in config.rs, read
 * straight from the Rust source so the frontend tests can never drift from it.
 */
export function rustEnumValues(enumName) {
    const match = configRs.match(
        new RegExp(`#\\[serde\\(rename_all = "([^"]+)"\\)\\]\\s*pub enum ${enumName} \\{([^}]*)\\}`),
    );
    if (!match) throw new Error(`enum ${enumName} not found in config.rs`);
    const [, rule, body] = match;
    const rename = RENAMES[rule];
    if (!rename) throw new Error(`unsupported serde rename rule ${rule}`);
    return body
        .split(",")
        .map((v) => v.trim())
        .filter((v) => /^[A-Z]\w*$/.test(v)) // unit variants only
        .map(rename);
}
