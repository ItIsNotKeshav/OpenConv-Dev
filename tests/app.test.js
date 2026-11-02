import { describe, expect, it } from "vitest";
import { loadApp } from "./helpers/load-app.js";

const SOURCE = "/home/me/Videos/holiday.mov";
const OUT_DIR = "/home/me/Converted";

/** Pick a source file and an output format in the Convert tab. */
async function chooseFileAndFormat(app, formatName, source = SOURCE) {
    app.$("conv-file-input").parentElement.click();
    await app.flush();
    app.$("conv-dropdown-trigger").click();
    const rowsText = () => Array.from(app.document.querySelectorAll(".dropdown-option-item"));
    // Popular formats are listed first; search for the rest.
    let row = rowsText().find((r) => r.textContent.startsWith(formatName));
    if (!row) {
        const search = app.$("conv-search-box");
        search.value = formatName.toLowerCase();
        search.dispatchEvent(new app.window.Event("input"));
        row = rowsText().find((r) => r.textContent.startsWith(formatName));
    }
    if (!row) throw new Error(`format ${formatName} not in picker (source ${source})`);
    row.click();
}

/** Click Convert and return the config sent to the backend. */
async function submit(app, button = "conv-submit-btn") {
    app.$(button).click();
    await app.flush();
    const call = app.invoke.mock.calls.find(([cmd]) => cmd === "start_conversion");
    expect(call, "start_conversion was not invoked").toBeTruthy();
    return call[1].config;
}

describe("convert flow", () => {
    it("is disabled until a file and format are chosen", async () => {
        const app = loadApp({ dialogResults: [SOURCE] });
        expect(app.$("conv-submit-btn").disabled).toBe(true);
        app.$("conv-file-input").parentElement.click();
        await app.flush();
        expect(app.$("conv-source-name").textContent).toBe("holiday.mov");
        expect(app.$("conv-submit-btn").disabled).toBe(true);
    });

    it("sends an MP4 conversion with the chosen folder", async () => {
        const app = loadApp({ dialogResults: [SOURCE, OUT_DIR] });
        await chooseFileAndFormat(app, "MP4");
        expect(app.$("conv-target-ext").textContent).toBe(".mp4");
        expect(app.$("conv-submit-btn").disabled).toBe(false);

        const config = await submit(app);
        expect(config).toMatchObject({
            inputPath: SOURCE,
            outputDir: OUT_DIR,
            container: "mp4",
            videoCodec: "libx264",
            audioCodec: "aac",
            resolution: "source",
            frameRate: "source",
            hardwareAcceleration: "off",
            overwrite: false,
        });
        expect(app.$("conv-submit-btn").disabled).toBe(true);
        expect(app.$("conv-save-location").textContent).toBe("Converted");
    });

    it("writes WebM as .webm, not Matroska", async () => {
        const app = loadApp({ dialogResults: [SOURCE, OUT_DIR] });
        await chooseFileAndFormat(app, "WebM");
        const config = await submit(app);
        expect(config).toMatchObject({ container: "webm", videoCodec: "libvpx-vp9", audioCodec: "opus" });
    });

    it.each([
        ["PNG", "png"],
        ["GIF", "gif"],
        ["WebP", "webp"],
    ])("sends no codecs for %s images", async (name, container) => {
        const app = loadApp({ dialogResults: [SOURCE, OUT_DIR] });
        await chooseFileAndFormat(app, name);
        const config = await submit(app);
        expect(config).toMatchObject({ container, videoCodec: null, audioCodec: null });
    });

    it.each([
        ["MP3", "mp3", "mp3"],
        ["FLAC", "flac", "flac"],
        ["M4A", "m4a", "aac"],
    ])("sends no video codec for %s audio", async (name, container, audioCodec) => {
        const app = loadApp({ dialogResults: [SOURCE, OUT_DIR] });
        await chooseFileAndFormat(app, name);
        const config = await submit(app);
        expect(config).toMatchObject({ container, videoCodec: null, audioCodec });
    });

    it("keeps video codecs null for audio formats after a video setting changes", async () => {
        const app = loadApp({ dialogResults: [SOURCE, OUT_DIR] });
        await chooseFileAndFormat(app, "MP3");
        app.select("conv-video-codec", "libx265");
        const config = await submit(app);
        expect(config.videoCodec).toBe(null);
    });

    it("passes through the advanced settings", async () => {
        const app = loadApp({ dialogResults: [SOURCE, OUT_DIR] });
        await chooseFileAndFormat(app, "MKV");
        app.select("conv-video-codec", "libx265");
        app.select("conv-resolution", "1080p");
        app.select("conv-frame-rate", "30");
        app.select("conv-preset", "slow");
        app.select("conv-audio-codec", "opus");
        app.select("conv-audio-bitrate", "192k");
        app.select("conv-channels", "stereo");
        app.select("conv-sample-rate", "48000");
        app.$("conv-overwrite").checked = true;
        app.$("conv-overwrite").dispatchEvent(new app.window.Event("change"));
        const extra = app.$("conv-extra-args");
        extra.value = "-movflags +faststart";
        extra.dispatchEvent(new app.window.Event("input"));

        const config = await submit(app);
        expect(config).toMatchObject({
            container: "matroska",
            videoCodec: "libx265",
            resolution: "p1080",
            frameRate: "fps30",
            preset: "slow",
            audioCodec: "opus",
            audioBitrate: "192k",
            channels: "stereo",
            sampleRate: "hz48000",
            overwrite: true,
            extraArguments: "-movflags +faststart",
        });
    });

    it("only offers audio formats for an audio source", async () => {
        const app = loadApp({ dialogResults: ["/music/song.flac"] });
        app.$("conv-file-input").parentElement.click();
        await app.flush();
        app.$("conv-dropdown-trigger").click();
        const names = Array.from(app.document.querySelectorAll(".dropdown-option-item")).map((r) => r.textContent);
        expect(names.length).toBeGreaterThan(0);
        expect(names.some((n) => n.startsWith("MP4"))).toBe(false);
        expect(names.some((n) => n.startsWith("MP3"))).toBe(true);
    });

    it("never lists formats the backend cannot produce", async () => {
        const app = loadApp({ dialogResults: [SOURCE] });
        app.$("conv-file-input").parentElement.click();
        await app.flush();
        app.$("conv-dropdown-trigger").click();
        const search = app.$("conv-search-box");
        search.value = "wmv";
        search.dispatchEvent(new app.window.Event("input"));
        expect(app.$("conv-options-container").textContent).toContain("No matches found");
    });

    it("accepts common media files in the file picker", async () => {
        const app = loadApp({ dialogResults: [null] });
        app.$("conv-file-input").parentElement.click();
        await app.flush();
        const [{ filters }] = app.openDialog.mock.calls[0];
        for (const ext of ["mp4", "webm", "m4a", "ogg", "gif", "jpg"]) {
            expect(filters[0].extensions).toContain(ext);
        }
    });

    it("does nothing when the folder dialog is cancelled", async () => {
        const app = loadApp({ dialogResults: [SOURCE, null] });
        await chooseFileAndFormat(app, "MP4");
        app.$("conv-submit-btn").click();
        await app.flush();
        expect(app.invoke).not.toHaveBeenCalledWith("start_conversion", expect.anything());
        expect(app.$("conv-submit-btn").disabled).toBe(false);
    });

    it("shows backend validation errors and re-enables the button", async () => {
        const app = loadApp({ dialogResults: [SOURCE, OUT_DIR] });
        app.invoke.mockRejectedValueOnce("The selected input file no longer exists.");
        await chooseFileAndFormat(app, "MP4");
        app.$("conv-submit-btn").click();
        await app.flush();
        await app.flush();
        expect(app.$("conv-target-status").textContent).toBe("The selected input file no longer exists.");
        expect(app.$("conv-submit-btn").disabled).toBe(false);
        expect(app.$("conv-submit-btn").textContent).toBe("Convert");
    });
});

describe("compress flow", () => {
    it("sends an MP4 with the compression settings", async () => {
        const app = loadApp({ dialogResults: [SOURCE, OUT_DIR] });
        app.$("tab-compress").click();
        app.$("comp-file-input").parentElement.click();
        await app.flush();
        app.$("comp-audio-mono").checked = true;
        app.$("comp-audio-mono").dispatchEvent(new app.window.Event("change"));
        const crf = app.$("comp-video-crf");
        crf.value = "28";
        crf.dispatchEvent(new app.window.Event("input"));

        const config = await submit(app, "comp-submit-btn");
        expect(config).toMatchObject({ container: "mp4", crf: 28, channels: "mono", inputPath: SOURCE });
        expect(app.$("comp-crf-val").textContent).toBe("28");
    });
});

describe("progress events", () => {
    async function startedApp() {
        const app = loadApp({ dialogResults: [SOURCE, OUT_DIR] });
        await chooseFileAndFormat(app, "MP4");
        await submit(app);
        app.emit("conversion_started", { filename: "holiday.mov", outputPath: `${OUT_DIR}/holiday.mp4` });
        return app;
    }

    it("shows the encoder card while running", async () => {
        const app = await startedApp();
        expect(app.$("conv-output-panel").dataset.convState).toBe("encoding");
        expect(app.$("conv-enc-filename").textContent).toBe("holiday.mov");

        app.emit("ffmpeg_progress", {
            percentage: 42.4,
            elapsed: "00:10",
            remaining: "00:14",
            remainingSecs: 14,
            fps: "60",
            speed: "2.1x",
            bitrate: "1200 kbps",
            outputSize: "3.1 MB",
            stage: "Encoding Video",
            encoder: "H.264",
        });
        expect(app.$("conv-enc-pct").textContent).toBe("42%");
        expect(app.$("conv-enc-bar").style.width).toBe("42%");
        expect(app.$("conv-enc-stage").textContent).toBe("Encoding Video");
        expect(app.$("conv-enc-encoder").textContent).toBe("H.264");
        expect(app.$("conv-enc-eta").textContent).not.toBe("—");
        expect(app.$("conv-progress").textContent).toBe("42%");
        expect(app.$("conv-speed").textContent).toBe("60 fps / 2.1x");
    });

    it("shows the result card when done", async () => {
        const app = await startedApp();
        app.emit("conversion_complete", {
            outputPath: `${OUT_DIR}/holiday.mp4`,
            originalSize: 1000,
            outputSize: 250,
            elapsedSecs: 75,
            message: "Saved to: x",
        });
        expect(app.$("conv-output-panel").dataset.convState).toBe("done");
        expect(app.$("conv-done-savings").textContent).toBe("–75.0% saved");
        expect(app.$("conv-done-time").textContent).toBe("1m 15s");
        expect(app.$("conv-submit-btn").disabled).toBe(false);

        app.$("conv-done-folder").click();
        expect(app.invoke).toHaveBeenCalledWith("open_in_explorer", { path: `${OUT_DIR}/holiday.mp4` });
        app.$("conv-done-file").click();
        expect(app.invoke).toHaveBeenCalledWith("open_file", { path: `${OUT_DIR}/holiday.mp4` });
        app.$("conv-done-again").click();
        expect(app.$("conv-output-panel").dataset.convState).toBe("");
    });

    it("shows the error message on failure", async () => {
        const app = await startedApp();
        app.emit("conversion_failed", { message: "The destination disk is full.", stderr: "..." });
        expect(app.$("conv-output-panel").dataset.convState).toBe("error");
        expect(app.$("conv-enc-error").textContent).toBe("The destination disk is full.");
        expect(app.$("conv-enc-stage").textContent).toBe("Failed");
        expect(app.$("conv-submit-btn").disabled).toBe(false);
    });

    it("cancels through the backend", async () => {
        const app = await startedApp();
        app.$("conv-cancel-btn").click();
        expect(app.invoke).toHaveBeenCalledWith("cancel_conversion");
        app.emit("conversion_cancelled", null);
        expect(app.$("conv-target-status").textContent).toBe("Conversion cancelled.");
        expect(app.$("conv-progress").textContent).toBe("0%");
    });

    it("keeps updating the panel that started the run after switching tabs", async () => {
        const app = await startedApp();
        app.$("tab-compress").click();
        app.emit("ffmpeg_progress", { percentage: 80, stage: "Encoding Video" });
        expect(app.$("conv-enc-pct").textContent).toBe("80%");
        expect(app.$("comp-enc-pct").textContent).not.toBe("80%");
        app.emit("conversion_complete", { outputPath: "x", originalSize: 1, outputSize: 1 });
        expect(app.$("conv-output-panel").dataset.convState).toBe("done");
        expect(app.$("comp-output-panel").dataset.convState ?? "").not.toBe("done");
    });
});

describe("outside the desktop app", () => {
    it("explains that conversion needs Tauri", async () => {
        const app = loadApp({ tauri: false });
        const input = app.$("conv-file-input");
        const file = new app.window.File(["data"], "clip.mp4");
        Object.defineProperty(input, "files", { value: [file] });
        input.dispatchEvent(new app.window.Event("change"));
        app.get("applyFormatPreset")("mp4");
        app.$("conv-submit-btn").click();
        await app.flush();
        expect(app.window.alert).toHaveBeenCalled();
    });
});

describe("theme", () => {
    it("remembers the chosen theme", () => {
        const app = loadApp();
        const light = app.document.querySelector('[data-theme-choice="light"]');
        light.click();
        expect(app.document.documentElement.dataset.theme).toBe("light");
        expect(app.window.localStorage.getItem("openconv-theme")).toBe("light");
    });
});

describe("formatting helpers", () => {
    const app = loadApp();

    it("formats byte sizes", () => {
        const formatBytes = app.get("formatBytes");
        expect(formatBytes(undefined)).toBe("—");
        expect(formatBytes(0)).toBe("—");
        expect(formatBytes(512)).toBe("512 B");
        expect(formatBytes(1536)).toBe("1.5 KB");
        expect(formatBytes(5 * 1024 ** 3)).toBe("5.0 GB");
    });

    it("formats elapsed times", () => {
        const formatElapsed = app.get("formatElapsed");
        expect(formatElapsed(0)).toBe("0s");
        expect(formatElapsed(59.6)).toBe("1m 0s");
        expect(formatElapsed(3725)).toBe("1h 2m 5s");
    });

    it("formats the ETA in local time", () => {
        const formatEta = app.get("formatEta");
        expect(formatEta(undefined)).toBe("—");
        expect(formatEta(0)).toBe("—");
        const now = new Date(2026, 0, 1, 13, 0, 0).getTime();
        const expected = new Date(now + 90 * 60 * 1000).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
        expect(formatEta(90 * 60, now)).toBe(expected);
    });
});
