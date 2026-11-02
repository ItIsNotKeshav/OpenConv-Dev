import { describe, expect, it } from "vitest";
import { loadApp } from "./helpers/load-app.js";
import { rustEnumValues } from "./helpers/rust-enums.js";

const containers = rustEnumValues("Container");
const videoCodecs = rustEnumValues("VideoCodec");
const audioCodecs = rustEnumValues("AudioCodec");

describe("Rust enum parsing (sanity check for the helpers)", () => {
    it("reads the serde names the backend accepts", () => {
        expect(containers).toContain("mp4");
        expect(containers).toContain("matroska");
        expect(containers).toContain("webm");
        expect(videoCodecs).toEqual(["libx264", "libx265", "libvpx-vp9", "av1", "copy"]);
        expect(audioCodecs).toEqual(["aac", "mp3", "opus", "flac", "pcm", "copy"]);
        expect(rustEnumValues("Resolution")).toEqual(["source", "p720", "p1080", "p1440", "p2160"]);
    });
});

describe("format presets", () => {
    const app = loadApp();
    const presets = app.get("FORMAT_PRESETS");
    const available = app.get("AVAILABLE_FORMATS");
    const supported = app.get("SUPPORTED_FORMATS");

    it.each(Object.entries(presets))("%s preset only uses values the backend accepts", (ext, preset) => {
        expect(containers).toContain(preset.container);
        if (preset.videoCodec !== null) expect(videoCodecs).toContain(preset.videoCodec);
        if (preset.audioCodec !== null) expect(audioCodecs).toContain(preset.audioCodec);
        expect(["video", "audio", "image"]).toContain(preset.kind);
    });

    it("writes files with the extension the user picked", () => {
        // The backend names the file after the container, so a preset must not
        // map an extension to a different container (e.g. .webm -> .mkv).
        const containerExt = { matroska: "mkv" };
        for (const [ext, preset] of Object.entries(presets)) {
            expect(containerExt[preset.container] ?? preset.container, ext).toBe(ext);
        }
    });

    it("lists each supported format once, with a matching kind", () => {
        const exts = supported.map((f) => f.ext);
        expect(new Set(exts).size).toBe(exts.length);
        for (const format of supported) {
            expect(presets[format.ext].kind, format.ext).toBe(format.type);
        }
    });

    it("offers every preset in the format picker", () => {
        expect(supported.map((f) => f.ext).sort()).toEqual(Object.keys(presets).sort());
    });

    it("keeps unsupported entries of formats.js out of the picker", () => {
        const unsupported = available.filter((f) => !presets[f.ext]).map((f) => f.ext);
        expect(unsupported).toContain("wmv");
        for (const ext of unsupported) {
            expect(supported.some((f) => f.ext === ext), ext).toBe(false);
        }
    });

    it("has unique extensions in formats.js", () => {
        const exts = available.map((f) => f.ext);
        expect(new Set(exts).size).toBe(exts.length);
    });
});

describe("settings dropdowns", () => {
    const app = loadApp();
    const optionValues = (id) => Array.from(app.$(id).options).map((o) => o.value);

    it.each(["conv-video-codec", "comp-video-codec"])("%s only offers known video codecs", (id) => {
        for (const value of optionValues(id)) expect(videoCodecs).toContain(value);
    });

    it.each(["conv-audio-codec", "comp-audio-codec"])("%s only offers known audio codecs", (id) => {
        for (const value of optionValues(id)) expect(audioCodecs).toContain(value);
    });

    it.each([
        ["conv-resolution", "mapResolution", "Resolution"],
        ["comp-video-res", "mapResolution", "Resolution"],
        ["conv-frame-rate", "mapFrameRate", "FrameRate"],
        ["comp-frame-rate", "mapFrameRate", "FrameRate"],
        ["conv-sample-rate", "mapSampleRate", "SampleRate"],
        ["comp-sample-rate", "mapSampleRate", "SampleRate"],
        ["conv-channels", "mapChannels", "Channels"],
    ])("%s maps every option through %s to a valid %s", (id, mapper, enumName) => {
        const allowed = rustEnumValues(enumName);
        const map = app.get(mapper);
        for (const value of optionValues(id)) {
            expect(allowed, `${id}=${value}`).toContain(map(value));
        }
    });

    it.each(["conv-preset", "comp-preset"])("%s only offers known presets", (id) => {
        const allowed = rustEnumValues("Preset");
        for (const value of optionValues(id)) expect(allowed).toContain(value);
    });
});
