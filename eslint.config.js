import js from "@eslint/js";
import globals from "globals";

export default [
    {
        ignores: ["node_modules/", "src-tauri/target/"],
    },
    js.configs.recommended,
    {
        // The UI scripts are plain browser scripts loaded by index.html:
        // formats.js defines AVAILABLE_FORMATS, app.js uses it.
        files: ["src/**/*.js"],
        languageOptions: {
            ecmaVersion: 2022,
            sourceType: "script",
            globals: globals.browser,
        },
        rules: {
            "no-unused-vars": ["error", { varsIgnorePattern: "^AVAILABLE_FORMATS$" }],
            eqeqeq: ["error", "smart"],
            "no-var": "error",
            "prefer-const": "error",
        },
    },
    {
        files: ["src/app.js"],
        languageOptions: {
            globals: { AVAILABLE_FORMATS: "readonly" },
        },
    },
    {
        files: ["tests/**/*.js", "*.config.js"],
        languageOptions: {
            ecmaVersion: 2022,
            sourceType: "module",
            globals: { ...globals.node },
        },
    },
];
