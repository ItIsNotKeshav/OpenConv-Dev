import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { JSDOM } from "jsdom";
import { vi } from "vitest";

const srcDir = fileURLToPath(new URL("../../src/", import.meta.url));
const read = (name) => readFileSync(srcDir + name, "utf8");

/**
 * Load src/index.html with formats.js and app.js in a fresh jsdom window.
 *
 * With `tauri: true` a fake `window.__TAURI__` records every `invoke` call,
 * answers dialogs with `dialogResults` (in order) and lets tests fire backend
 * events through `emit(name, payload)`.
 */
export function loadApp({ tauri = true, dialogResults = [] } = {}) {
    // Strip the <script src> tags; the scripts are evaluated below so the fake
    // Tauri API can be installed first.
    const html = read("index.html").replace(/<script src="[^"]+"><\/script>/g, "");
    const dom = new JSDOM(html, { url: "http://localhost/", runScripts: "dangerously", pretendToBeVisual: true });
    const { window } = dom;

    window.matchMedia = () => ({ matches: true, addEventListener() {}, removeEventListener() {} });
    window.alert = vi.fn();

    const listeners = {};
    const invoke = vi.fn(async () => undefined);
    const dialogQueue = [...dialogResults];
    const openDialog = vi.fn(async () => dialogQueue.shift() ?? null);

    if (tauri) {
        window.__TAURI__ = {
            tauri: { invoke },
            dialog: { open: openDialog },
            event: {
                listen: (name, handler) => {
                    (listeners[name] ||= []).push(handler);
                    return Promise.resolve(() => {});
                },
            },
        };
    }

    // Run the scripts as real <script> elements so their top-level `const`s
    // share one global scope, exactly as in the app.
    for (const name of ["formats.js", "app.js"]) {
        const script = window.document.createElement("script");
        script.textContent = read(name);
        window.document.body.appendChild(script);
    }

    const $ = (id) => window.document.getElementById(id);
    return {
        window,
        document: window.document,
        $,
        invoke,
        openDialog,
        /** Evaluate an expression in the app's global scope (sees its `const`s). */
        get: (expr) => window.eval(expr),
        emit: (name, payload) => (listeners[name] || []).forEach((handler) => handler({ payload })),
        /** Let pending promises (dialogs, invoke) settle. */
        flush: () => new Promise((resolve) => setTimeout(resolve, 0)),
        select(id, value) {
            const el = $(id);
            el.value = value;
            el.dispatchEvent(new window.Event("change"));
        },
    };
}
