const $ = (id) => document.getElementById(id);
const tabConvert = $("tab-convert");
const tabCompress = $("tab-compress");
const panelConvert = $("panel-convert");
const panelCompress = $("panel-compress");
const themeToggle = $("theme-toggle");
const themeOptions = Array.from(document.querySelectorAll(".theme-option"));
const themeMedia = window.matchMedia("(prefers-color-scheme: dark)");
const isTauri = window.__TAURI__ != null;
const invoke = isTauri ? window.__TAURI__.tauri.invoke : null;
const openDialog = isTauri ? window.__TAURI__.dialog.open : null;
const listen = isTauri && window.__TAURI__.event ? window.__TAURI__.event.listen : null;

const FORMAT_PRESETS = {
    mp4: { container: "mp4", videoCodec: "libx264", audioCodec: "aac", kind: "video", label: "MP4" },
    mov: { container: "mov", videoCodec: "libx264", audioCodec: "aac", kind: "video", label: "MOV" },
    mkv: { container: "matroska", videoCodec: "libx264", audioCodec: "aac", kind: "video", label: "MKV" },
    avi: { container: "avi", videoCodec: "libx264", audioCodec: "aac", kind: "video", label: "AVI" },
    webm: { container: "webm", videoCodec: "libvpx-vp9", audioCodec: "opus", kind: "video", label: "WEBM" },
    mp3: { container: "mp3", videoCodec: null, audioCodec: "mp3", kind: "audio", label: "MP3" },
    wav: { container: "wav", videoCodec: null, audioCodec: "pcm", kind: "audio", label: "WAV" },
    flac: { container: "flac", videoCodec: null, audioCodec: "flac", kind: "audio", label: "FLAC" },
    aac: { container: "aac", videoCodec: null, audioCodec: "aac", kind: "audio", label: "AAC" },
    m4a: { container: "m4a", videoCodec: null, audioCodec: "aac", kind: "audio", label: "M4A" },
    ogg: { container: "ogg", videoCodec: null, audioCodec: "opus", kind: "audio", label: "OGG" },
    opus: { container: "opus", videoCodec: null, audioCodec: "opus", kind: "audio", label: "OPUS" },
    png: { container: "png", videoCodec: null, audioCodec: null, kind: "image", label: "PNG" },
    webp: { container: "webp", videoCodec: null, audioCodec: null, kind: "image", label: "WEBP" },
    gif: { container: "gif", videoCodec: null, audioCodec: null, kind: "image", label: "GIF" },
};

const state = {
    activeMode: "convert",
    convert: createDefaultConfig(),
    compress: {
        ...createDefaultConfig(),
        container: "mp4",
        videoCodec: "libx264",
        audioCodec: "aac",
        crf: 23,
        audioBitrate: "128k",
    },
};

const controls = {
    convert: {
        uploadCard: $("conv-upload-card"),
        fileInput: $("conv-file-input"),
        sourceName: $("conv-source-name"),
        sourceExt: $("conv-source-ext"),
        sourceSize: $("conv-source-size"),
        sourceDuration: $("conv-source-duration"),
        sourceResolution: $("conv-source-resolution"),
        sourceCodec: $("conv-source-codec"),
        sourceFps: $("conv-source-fps"),
        sourceAudio: $("conv-source-audio"),
        targetName: $("conv-target-ext"),
        status: $("conv-target-status"),
        submit: $("conv-submit-btn"),
        download: $("conv-download-btn"),
        progress: $("conv-progress"),
        elapsed: $("conv-elapsed"),
        remaining: $("conv-remaining"),
        speed: $("conv-speed"),
        size: $("conv-size"),
        saveLocation: $("conv-save-location"),
        videoSection: panelConvert.querySelector(".settings-section:nth-of-type(2)"),
        audioSection: panelConvert.querySelector(".settings-section:nth-of-type(3)"),
    },
    compress: {
        uploadCard: $("comp-upload-card"),
        fileInput: $("comp-file-input"),
        sourceName: $("comp-source-name"),
        sourceExt: $("comp-source-ext"),
        sourceSize: $("comp-source-size"),
        sourceDuration: $("comp-source-duration"),
        sourceResolution: $("comp-source-resolution"),
        sourceCodec: $("comp-source-codec"),
        sourceFps: $("comp-source-fps"),
        sourceAudio: $("comp-source-audio"),
        targetName: null,
        status: $("comp-target-status"),
        submit: $("comp-submit-btn"),
        download: $("comp-download-btn"),
        progress: $("comp-progress"),
        elapsed: $("comp-elapsed"),
        remaining: $("comp-remaining"),
        speed: $("comp-speed"),
        size: $("comp-size"),
        saveLocation: $("comp-save-location"),
        videoSection: panelCompress.querySelector(".settings-section:nth-of-type(1)"),
        audioSection: panelCompress.querySelector(".settings-section:nth-of-type(2)"),
    },
};

function createDefaultConfig() {
    return {
        inputPath: "",
        outputPath: null,
        outputDir: null,
        container: null,
        kind: "video",
        videoCodec: "libx264",
        audioCodec: "aac",
        resolution: "source",
        frameRate: "source",
        preset: "medium",
        crf: 23,
        videoBitrate: null,
        audioBitrate: "128k",
        channels: "source",
        sampleRate: "source",
        hardwareAcceleration: "off",
        overwrite: false,
        extraArguments: "",
    };
}

function applyTheme(choice) {
    const selected = choice || localStorage.getItem("openconv-theme") || "dark";
    const resolved = selected === "system" ? (themeMedia.matches ? "dark" : "light") : selected;
    document.documentElement.dataset.theme = resolved;
    localStorage.setItem("openconv-theme", selected);
    themeOptions.forEach((button, index) => {
        const active = button.dataset.themeChoice === selected;
        button.classList.toggle("active", active);
        if (active) themeToggle.style.setProperty("--theme-index", index);
    });
}

themeOptions.forEach((button) => button.addEventListener("click", () => applyTheme(button.dataset.themeChoice)));
themeMedia.addEventListener("change", () => {
    if ((localStorage.getItem("openconv-theme") || "dark") === "system") applyTheme("system");
});
applyTheme(localStorage.getItem("openconv-theme") || "dark");

tabConvert.addEventListener("click", () => switchTab("convert"));
tabCompress.addEventListener("click", () => switchTab("compress"));

function switchTab(mode) {
    state.activeMode = mode;
    tabCompress.classList.toggle("active", mode === "compress");
    tabConvert.classList.toggle("active", mode === "convert");
    panelCompress.classList.toggle("active-view", mode === "compress");
    panelConvert.classList.toggle("active-view", mode === "convert");
    // Toggle which output panel is visible in the right rail
    const convOut = $("conv-output-panel");
    const compOut = $("comp-output-panel");
    if (convOut) convOut.classList.toggle("active-output", mode === "convert");
    if (compOut) compOut.classList.toggle("active-output", mode === "compress");
}

const convDropdownTrigger = $("conv-dropdown-trigger");
const convDropdownMenu = $("conv-dropdown-menu");
const convSearchBox = $("conv-search-box");
const convOptionsContainer = $("conv-options-container");
let convInputType = null;

convDropdownTrigger.addEventListener("click", () => toggleFormatMenu());
convDropdownTrigger.addEventListener("keydown", (e) => {
    if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        toggleFormatMenu();
    }
});

function toggleFormatMenu() {
    const visible = convDropdownMenu.style.display === "flex";
    if (visible) {
        convDropdownMenu.style.display = "none";
        return;
    }
    convDropdownMenu.style.display = "flex";
    convSearchBox.value = "";
    renderFormatsList();
    convSearchBox.focus();
}

document.addEventListener("click", (e) => {
    const wrapper = $("conv-dropdown-wrapper");
    if (wrapper && !wrapper.contains(e.target)) convDropdownMenu.style.display = "none";
});

convSearchBox.addEventListener("input", () => renderFormatsList(convSearchBox.value.trim().toLowerCase()));

// Only offer formats the backend can actually produce (see FORMAT_PRESETS).
const SUPPORTED_FORMATS = AVAILABLE_FORMATS.filter((format) => FORMAT_PRESETS[format.ext]);

function renderFormatsList(query = "") {
    convOptionsContainer.innerHTML = "";
    let pool = SUPPORTED_FORMATS;
    if (convInputType === "audio") pool = pool.filter((format) => format.type === "audio");

    const matches =
        query === ""
            ? pool.filter((format) => format.popular)
            : pool.filter((format) => format.ext.includes(query) || format.name.toLowerCase().includes(query));

    if (matches.length === 0) {
        convOptionsContainer.innerHTML = '<div class="dropdown-category-title" style="text-align:center;">No matches found</div>';
        return;
    }

    if (query === "") appendHead("Popular formats");
    ["video", "audio", "image"].forEach((type) => {
        const group = matches.filter((format) => format.type === type);
        if (group.length > 0) {
            if (query !== "") appendHead(type[0].toUpperCase() + type.slice(1));
            group.forEach((item) => appendRow(item));
        }
    });
}

function appendHead(text) {
    const heading = document.createElement("div");
    heading.className = "dropdown-category-title";
    heading.textContent = text;
    convOptionsContainer.appendChild(heading);
}

function appendRow(item) {
    const row = document.createElement("div");
    row.className = "dropdown-option-item";
    row.textContent = item.name;
    row.addEventListener("click", () => {
        applyFormatPreset(item.ext);
        convDropdownMenu.style.display = "none";
    });
    convOptionsContainer.appendChild(row);
}

function applyFormatPreset(ext) {
    const preset = FORMAT_PRESETS[ext];
    if (!preset) return;
    Object.assign(state.convert, {
        container: preset.container,
        kind: preset.kind,
        videoCodec: preset.videoCodec,
        audioCodec: preset.audioCodec,
    });
    setSelect("conv-video-codec", preset.videoCodec || "libx264");
    setSelect("conv-audio-codec", preset.audioCodec || "aac");
    convDropdownTrigger.textContent = preset.label;
    controls.convert.targetName.textContent = `.${ext}`;
    controls.convert.status.textContent = `Output will be written as .${ext}.`;
    syncConfigFromControls("convert");
    updateRelevantSections("convert", preset.kind);
    updateSubmitState("convert");
}

function setSelect(id, value) {
    const select = $(id);
    if (select && value != null) select.value = value;
}

const VIDEO_INPUTS = ["mp4", "mkv", "avi", "mov", "webm", "m4v", "wmv", "flv", "mpeg", "mpg", "ts", "3gp", "ogv", "vob"];
const AUDIO_INPUTS = ["mp3", "wav", "flac", "aac", "m4a", "ogg", "opus", "wma", "aiff", "amr", "ac3"];
const IMAGE_INPUTS = ["png", "jpg", "jpeg", "gif", "webp", "bmp"];

bindFilePicker("convert", [...VIDEO_INPUTS, ...AUDIO_INPUTS, ...IMAGE_INPUTS]);
bindFilePicker("compress", VIDEO_INPUTS);
bindConfigControls();
bindProgressListener();
updateRelevantSections("convert", "video");
updateRelevantSections("compress", "video");

function bindFilePicker(mode, extensions) {
    const ui = controls[mode];
    const input = ui.fileInput;
    if (!input || !input.parentElement) return;

    if (isTauri) {
        input.parentElement.addEventListener("click", async (e) => {
            e.preventDefault();
            e.stopPropagation();
            const selectedFile = await openDialog({
                multiple: false,
                filters: [
                    { name: "Media Files", extensions },
                    { name: "All Files", extensions: ["*"] },
                ],
            });
            if (selectedFile) {
                handleFileSelection(mode, selectedFile, selectedFile.split("\\").pop().split("/").pop());
            }
        });
    } else {
        input.addEventListener("change", (e) => {
            if (e.target.files.length > 0) {
                const file = e.target.files[0];
                handleFileSelection(mode, file.name, file.name, file.size);
            }
        });
    }
    attachDropState(mode);
}

function handleFileSelection(mode, filePath, fileName, fileSize) {
    const ui = controls[mode];
    const config = state[mode];
    const ext = "." + fileName.split(".").pop();
    config.inputPath = filePath;

    ui.uploadCard.classList.add("has-file");
    ui.sourceName.textContent = fileName;
    ui.sourceExt.textContent = ext.toLowerCase();
    ui.sourceSize.textContent = formatBytes(fileSize);
    ui.sourceDuration.textContent = "From source";
    ui.sourceCodec.textContent = ext.replace(".", "").toUpperCase();
    ui.sourceResolution.textContent = "Original";
    ui.sourceFps.textContent = "Source";
    ui.sourceAudio.textContent = "Source";
    ui.status.textContent = mode === "convert" ? "Ready for a target format." : "Ready to compress into a smaller file.";
    if (ui.download) ui.download.style.display = "none";
    resetProgress(mode);

    if (mode === "convert") {
        convInputType = AUDIO_INPUTS.includes(ext.replace(".", "").toLowerCase()) ? "audio" : "video";
        config.container = null;
        config.kind = "video";
        convDropdownTrigger.textContent = "Target Type";
        controls.convert.targetName.textContent = "Choose format";
        updateRelevantSections("convert", "video");
        renderFormatsList();
    } else {
        config.container = "mp4";
        syncConfigFromControls("compress");
    }
    updateSubmitState(mode);
}

function bindConfigControls() {
    [
        "conv-video-codec",
        "conv-resolution",
        "conv-frame-rate",
        "conv-preset",
        "conv-audio-codec",
        "conv-audio-bitrate",
        "conv-channels",
        "conv-sample-rate",
        "conv-overwrite",
        "conv-hardware-accel",
        "conv-extra-args",
    ].forEach((id) => bindControl(id, () => syncConfigFromControls("convert")));

    [
        "comp-video-codec",
        "comp-video-res",
        "comp-frame-rate",
        "comp-preset",
        "comp-video-crf",
        "comp-audio-codec",
        "comp-audio-bitrate",
        "comp-audio-mono",
        "comp-sample-rate",
        "comp-overwrite",
        "comp-hardware-accel",
        "comp-extra-args",
    ].forEach((id) => bindControl(id, () => syncConfigFromControls("compress")));
}

function bindControl(id, handler) {
    const element = $(id);
    if (!element) return;
    const eventName = element.tagName === "TEXTAREA" || element.type === "range" ? "input" : "change";
    element.addEventListener(eventName, handler);
}

function syncConfigFromControls(mode) {
    const config = state[mode];
    if (mode === "convert") {
        Object.assign(config, {
            // Audio-only and image formats have no use for the codec that is
            // still selected in the other section's dropdown.
            videoCodec: config.kind === "video" ? valueOrNull("conv-video-codec") : null,
            audioCodec: config.kind === "image" ? null : valueOrNull("conv-audio-codec"),
            resolution: mapResolution($("conv-resolution").value),
            frameRate: mapFrameRate($("conv-frame-rate").value),
            preset: $("conv-preset").value,
            audioBitrate: valueOrNull("conv-audio-bitrate"),
            channels: mapChannels($("conv-channels").value),
            sampleRate: mapSampleRate($("conv-sample-rate").value),
            hardwareAcceleration: $("conv-hardware-accel").checked ? "auto" : "off",
            overwrite: $("conv-overwrite").checked,
            extraArguments: $("conv-extra-args").value,
        });
    } else {
        Object.assign(config, {
            container: "mp4",
            videoCodec: valueOrNull("comp-video-codec"),
            audioCodec: valueOrNull("comp-audio-codec"),
            resolution: mapResolution($("comp-video-res").value),
            frameRate: mapFrameRate($("comp-frame-rate").value),
            preset: $("comp-preset").value,
            crf: Number($("comp-video-crf").value),
            audioBitrate: valueOrNull("comp-audio-bitrate"),
            channels: $("comp-audio-mono").checked ? "mono" : "source",
            sampleRate: mapSampleRate($("comp-sample-rate").value),
            hardwareAcceleration: $("comp-hardware-accel").checked ? "auto" : "off",
            overwrite: $("comp-overwrite").checked,
            extraArguments: $("comp-extra-args").value,
        });
        $("comp-crf-val").textContent = config.crf;
    }
    updateSubmitState(mode);
}

function valueOrNull(id) {
    const value = $(id).value;
    return value === "" ? null : value;
}

function mapResolution(value) {
    return {
        source: "source",
        "720p": "p720",
        "1080p": "p1080",
        "1440p": "p1440",
        "2160p": "p2160",
    }[value] || "source";
}

function mapFrameRate(value) {
    return value === "source" ? "source" : `fps${value}`;
}

function mapChannels(value) {
    return value || "source";
}

function mapSampleRate(value) {
    return value === "source" ? "source" : `hz${value}`;
}

function updateRelevantSections(mode, kind) {
    const ui = controls[mode];
    const videoDisabled = kind === "audio";
    const audioDisabled = kind === "image";
    ui.videoSection.classList.toggle("is-disabled", videoDisabled);
    ui.audioSection.classList.toggle("is-disabled", audioDisabled);
}

function updateSubmitState(mode) {
    const ui = controls[mode];
    const config = state[mode];
    ui.submit.disabled = !config.inputPath || !config.container;
}

// ── Conversion output path tracking ─────────────────────────────
const conversionOutput = { convert: null, compress: null };
// Which panel started the current run, so events still reach it if the user
// switches tabs while FFmpeg is working.
let runningMode = null;

// ── Output panel state helpers ───────────────────────────────────
function setOutputPanelState(mode, convState) {
    const panelId = mode === "convert" ? "conv-output-panel" : "comp-output-panel";
    const panel = $(panelId);
    if (panel) panel.dataset.convState = convState;
}

// ── Non-blocking runConfig ───────────────────────────────────────
async function runConfig(mode) {
    const ui = controls[mode];
    const config = state[mode];
    const error = validateConfig(config);
    if (error) {
        ui.status.textContent = error;
        return;
    }
    if (!isTauri) {
        alert("This feature requires running inside the Tauri desktop app.");
        return;
    }

    const targetFolderDirectory = await openDialog({
        directory: true,
        multiple: false,
        title: mode === "convert" ? "Choose Destination Folder" : "Select Compression Output Folder",
    });
    if (!targetFolderDirectory) return;

    config.outputDir = targetFolderDirectory;
    ui.saveLocation.textContent = targetFolderDirectory.split("\\").pop().split("/").pop() || "Selected";

    // Disable submit so user can't double-start
    ui.submit.textContent = "Converting…";
    ui.submit.disabled = true;

    try {
        // Returns immediately — progress comes via events
        runningMode = mode;
        await invoke("start_conversion", { config: { ...config } });
    } catch (err) {
        runningMode = null;
        ui.status.textContent = String(err);
        ui.submit.textContent = mode === "convert" ? "Convert" : "Compress";
        updateSubmitState(mode);
    }
}

function validateConfig(config) {
    if (!config.inputPath) return "Choose a source file before starting.";
    if (!config.container) return "Choose an output format before starting.";
    return "";
}

$("conv-submit-btn").addEventListener("click", () => runConfig("convert"));
$("comp-submit-btn").addEventListener("click", () => runConfig("compress"));

// ── Event-driven progress system ─────────────────────────────────
function bindProgressListener() {
    if (!listen) return;

    listen("conversion_started", (event) => {
        const p = event.payload || {};
        const mode = runningMode || state.activeMode;
        const prefix = mode === "convert" ? "conv" : "comp";

        conversionOutput[mode] = p.outputPath || null;

        // Show encoder card
        setOutputPanelState(mode, "encoding");
        const filename = p.filename || "File";
        $(`${prefix}-enc-filename`).textContent = filename;
        $(`${prefix}-enc-stage`).textContent = "Analyzing…";
        $(`${prefix}-enc-pct`).textContent = "0%";
        $(`${prefix}-enc-bar`).style.width = "0%";
        resetEncoderStats(prefix);
        // Clear any previous error
        $(`${prefix}-enc-error`).textContent = "";
        $(`${prefix}-enc-error`).style.display = "none";
    });

    listen("ffmpeg_progress", (event) => {
        const p = event.payload || {};
        const mode = runningMode || state.activeMode;
        const prefix = mode === "convert" ? "conv" : "comp";
        updateEncoderCard(prefix, p);
        // Also keep the legacy stats panel in sync for non-encoding state
        updateProgress(mode, p);
    });

    listen("conversion_complete", (event) => {
        const p = event.payload || {};
        const mode = runningMode || state.activeMode;
        const prefix = mode === "convert" ? "conv" : "comp";
        const ui = controls[mode];

        conversionOutput[mode] = p.outputPath || conversionOutput[mode];

        // Transition to done state
        setOutputPanelState(mode, "done");

        // Populate completion card
        const origFmt = formatBytes(p.originalSize || 0);
        const outFmt  = formatBytes(p.outputSize  || 0);
        $(`${prefix}-done-orig`).textContent = origFmt;
        $(`${prefix}-done-out`).textContent  = outFmt;

        const savingsBadge = $(`${prefix}-done-savings`);
        if (p.originalSize > 0 && p.outputSize > 0 && p.originalSize > p.outputSize) {
            const pct = (((p.originalSize - p.outputSize) / p.originalSize) * 100).toFixed(1);
            savingsBadge.textContent = `–${pct}% saved`;
            savingsBadge.style.display = "inline-flex";
        } else {
            savingsBadge.style.display = "none";
        }

        $(`${prefix}-done-time`).textContent = formatElapsed(p.elapsedSecs || 0);
        $(`${prefix}-done-subtitle`).textContent = p.message || "Finished successfully";

        // Wire done-card buttons
        $(`${prefix}-done-folder`).onclick = () => isTauri && invoke("open_in_explorer", { path: p.outputPath || "" });
        $(`${prefix}-done-file`).onclick   = () => isTauri && invoke("open_file",        { path: p.outputPath || "" });
        $(`${prefix}-done-again`).onclick  = () => resetForNewConversion(mode);

        // Update legacy stats
        ui.progress.textContent = "100%";
        ui.status.textContent = p.message || "Conversion complete.";

        // Re-enable submit
        ui.submit.textContent = mode === "convert" ? "Convert" : "Compress";
        updateSubmitState(mode);
        runningMode = null;
    });

    listen("conversion_failed", (event) => {
        const p = event.payload || {};
        const mode = runningMode || state.activeMode;
        const prefix = mode === "convert" ? "conv" : "comp";
        const ui = controls[mode];

        setOutputPanelState(mode, "error");

        const errBox = $(`${prefix}-enc-error`);
        errBox.textContent = p.message || "Conversion failed.";
        errBox.style.display = "block";
        $(`${prefix}-enc-stage`).textContent = "Failed";

        ui.status.textContent = p.message || "Conversion failed.";
        ui.submit.textContent = mode === "convert" ? "Convert" : "Compress";
        updateSubmitState(mode);
        runningMode = null;
    });

    listen("conversion_cancelled", () => {
        const mode = runningMode || state.activeMode;
        const ui = controls[mode];
        setOutputPanelState(mode, "");
        ui.status.textContent = "Conversion cancelled.";
        ui.submit.textContent = mode === "convert" ? "Convert" : "Compress";
        updateSubmitState(mode);
        resetProgress(mode);
        runningMode = null;
    });
}

// ── Encoder card live update ──────────────────────────────────────
function updateEncoderCard(prefix, p) {
    const pct = Math.round(p.percentage || 0);
    $(`${prefix}-enc-pct`).textContent  = `${pct}%`;
    $(`${prefix}-enc-bar`).style.width  = `${pct}%`;
    $(`${prefix}-enc-stage`).textContent = p.stage || "Encoding…";
    $(`${prefix}-enc-elapsed`).textContent   = p.elapsed   || "00:00";
    $(`${prefix}-enc-remaining`).textContent = p.remaining || "—";
    $(`${prefix}-enc-speed`).textContent     = p.speed     || "—";
    $(`${prefix}-enc-fps`).textContent       = p.fps       ? `${p.fps}` : "—";
    $(`${prefix}-enc-bitrate`).textContent   = p.bitrate   || "—";
    $(`${prefix}-enc-size`).textContent      = p.outputSize || "—";
    $(`${prefix}-enc-encoder`).textContent   = p.encoder   || "—";
    $(`${prefix}-enc-eta`).textContent       = formatEta(p.remainingSecs);
}

function resetEncoderStats(prefix) {
    ["elapsed","remaining","speed","fps","bitrate","size","encoder","eta"].forEach(k => {
        $(`${prefix}-enc-${k}`).textContent = "—";
    });
}

// ── Cancel button wiring ─────────────────────────────────────────
if (isTauri) {
    $("conv-cancel-btn").addEventListener("click", () => invoke("cancel_conversion"));
    $("comp-cancel-btn").addEventListener("click", () => invoke("cancel_conversion"));
}

// ── Reset for a new conversion ───────────────────────────────────
function resetForNewConversion(mode) {
    setOutputPanelState(mode, "");
    const ui = controls[mode];
    ui.status.textContent = mode === "convert"
        ? "Select a source file and target format to prepare conversion."
        : "Select a source file to prepare compression.";
    resetProgress(mode);
    updateSubmitState(mode);
}

// ── Legacy progress stats (small grid in idle output panel) ──────
function updateProgress(mode, progress) {
    const ui = controls[mode];
    ui.progress.textContent = `${Math.round(progress.percentage || 0)}%`;
    ui.elapsed.textContent = progress.elapsed || "00:00";
    ui.remaining.textContent = progress.remaining || "--";
    ui.speed.textContent = [progress.fps ? `${progress.fps} fps` : null, progress.speed].filter(Boolean).join(" / ") || "--";
    ui.size.textContent = [progress.outputSize, progress.bitrate].filter(Boolean).join(" / ") || "After run";
}

function resetProgress(mode) {
    const ui = controls[mode];
    ui.progress.textContent = "0%";
    ui.elapsed.textContent = "00:00";
    ui.remaining.textContent = "--";
    ui.speed.textContent = "--";
    ui.size.textContent = "After run";
}

/** Wall-clock time (in the user's own time zone) `remainingSecs` from now. */
function formatEta(remainingSecs, now = Date.now()) {
    if (!Number.isFinite(remainingSecs) || remainingSecs <= 0) return "—";
    return new Date(now + remainingSecs * 1000).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
}

function formatElapsed(secs) {
    const s = Math.round(secs);
    const m = Math.floor(s / 60);
    const h = Math.floor(m / 60);
    if (h > 0) return `${h}h ${m % 60}m ${s % 60}s`;
    if (m > 0) return `${m}m ${s % 60}s`;
    return `${s}s`;
}

function formatBytes(bytes) {
    if (!Number.isFinite(bytes) || bytes <= 0) return "—";
    const units = ["B", "KB", "MB", "GB", "TB"];
    const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
    return `${(bytes / Math.pow(1024, index)).toFixed(index === 0 ? 0 : 1)} ${units[index]}`;
}

function attachDropState(mode) {
    const ui = controls[mode];
    const card = ui.uploadCard;
    if (!card) return;

    ["dragenter", "dragover"].forEach((eventName) => {
        card.addEventListener(eventName, (e) => {
            e.preventDefault();
            card.classList.add("is-dragging");
        });
    });

    ["dragleave", "drop"].forEach((eventName) => {
        card.addEventListener(eventName, (e) => {
            e.preventDefault();
            card.classList.remove("is-dragging");
        });
    });

    card.addEventListener("drop", (e) => {
        const file = e.dataTransfer && e.dataTransfer.files[0];
        if (!file || isTauri) return;
        handleFileSelection(mode, file.name, file.name, file.size);
        if (ui.fileInput) ui.fileInput.files = e.dataTransfer.files;
    });
}
