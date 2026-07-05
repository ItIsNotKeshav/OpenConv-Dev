//Add any formats here that you require
const AVAILABLE_FORMATS = [
  // VIDEO FORMATS
  { ext: "mp4", name: "MP4 (H.264 / AAC Video)", type: "video", popular: true },
  {
    ext: "mkv",
    name: "MKV (Matroska Container)",
    type: "video",
    popular: true,
  },
  {
    ext: "mov",
    name: "MOV (Apple QuickTime Movie)",
    type: "video",
    popular: true,
  },
  {
    ext: "webm",
    name: "WebM (VP9 / Web Optimized)",
    type: "video",
    popular: true,
  },
  {
    ext: "avi",
    name: "AVI (Audio Video Interleave)",
    type: "video",
    popular: false,
  },
  {
    ext: "wmv",
    name: "WMV (Windows Media Video)",
    type: "video",
    popular: false,
  },
  { ext: "flv", name: "FLV (Flash Video)", type: "video", popular: false },
  {
    ext: "m4v",
    name: "M4V (Apple Video Layout)",
    type: "video",
    popular: false,
  },
  {
    ext: "mpeg",
    name: "MPEG-2 (DVD/VCD Video File)",
    type: "video",
    popular: false,
  },
  {
    ext: "ts",
    name: "TS (MPEG Transport Stream)",
    type: "video",
    popular: false,
  },
  {
    ext: "3gp",
    name: "3GP (Legacy Mobile Video)",
    type: "video",
    popular: false,
  },
  { ext: "ogv", name: "OGV (Ogg Video)", type: "video", popular: false },
  { ext: "vob", name: "VOB (DVD Video Object)", type: "video", popular: false },

  // ==========================================
  // AUDIO FORMATS
  // ==========================================
  { ext: "mp3", name: "MP3 (MPEG Layer 3)", type: "audio", popular: true },
  {
    ext: "wav",
    name: "WAV (Uncompressed PCM Lossless)",
    type: "audio",
    popular: true,
  },
  {
    ext: "aac",
    name: "AAC (Advanced Audio Coding)",
    type: "audio",
    popular: true,
  },
  { ext: "m4a", name: "M4A (Apple Audio Codec)", type: "audio", popular: true },
  {
    ext: "flac",
    name: "FLAC (Free Lossless Audio)",
    type: "audio",
    popular: true,
  },
  {
    ext: "ogg",
    name: "OGG (Vorbis Audio Structure)",
    type: "audio",
    popular: false,
  },
  {
    ext: "opus",
    name: "Opus (Interactive Audio Codec)",
    type: "audio",
    popular: false,
  },
  {
    ext: "wma",
    name: "WMA (Windows Media Audio)",
    type: "audio",
    popular: false,
  },
  {
    ext: "amr",
    name: "AMR (Speech Audio Codec)",
    type: "audio",
    popular: false,
  },
  {
    ext: "aiff",
    name: "AIFF (Apple Audio Interchange)",
    type: "audio",
    popular: false,
  },
  {
    ext: "caf",
    name: "CAF (Core Audio Format)",
    type: "audio",
    popular: false,
  },
  {
    ext: "ac3",
    name: "AC3 (Dolby Digital Audio)",
    type: "audio",
    popular: false,
  },
  {
    ext: "mp2",
    name: "MP2 (MPEG Layer 2 Audio)",
    type: "audio",
    popular: false,
  },

  // ==========================================
  // IMAGE / SEQUENCES
  // ==========================================
  { ext: "gif", name: "GIF (Animated Graphic)", type: "image", popular: true },
  {
    ext: "png",
    name: "PNG (Image Frame Extraction)",
    type: "image",
    popular: false,
  },
  {
    ext: "jpg",
    name: "JPG (Thumbnail Frame Extraction)",
    type: "image",
    popular: false,
  },
  {
    ext: "webp",
    name: "WebP (Static/Animated Image)",
    type: "image",
    popular: false,
  },
];
