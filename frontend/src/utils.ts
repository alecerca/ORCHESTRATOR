// Small presentation helpers shared across components.

/** Turn anything thrown by `invoke()` into a readable string. */
export function describeError(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  if (error && typeof error === "object" && "message" in error) {
    return String((error as { message: unknown }).message);
  }
  try {
    return JSON.stringify(error);
  } catch {
    return String(error);
  }
}

const EXTENSION_LANGUAGE: Record<string, string> = {
  ts: "typescript",
  tsx: "typescript",
  js: "javascript",
  jsx: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  json: "json",
  rs: "rust",
  css: "css",
  scss: "scss",
  less: "less",
  html: "html",
  htm: "html",
  md: "markdown",
  markdown: "markdown",
  py: "python",
  go: "go",
  sh: "shell",
  bash: "shell",
  zsh: "shell",
  yml: "yaml",
  yaml: "yaml",
  toml: "ini",
  ini: "ini",
  sql: "sql",
  c: "c",
  h: "c",
  cpp: "cpp",
  cc: "cpp",
  hpp: "cpp",
  java: "java",
  rb: "ruby",
  php: "php",
  xml: "xml",
  swift: "swift",
  kt: "kotlin",
  lua: "lua",
  dockerfile: "dockerfile",
  makefile: "makefile",
};

/** Best-effort Monaco language id from a path's extension. */
export function languageForPath(path: string): string {
  const name = path.split("/").pop() ?? path;
  const dot = name.lastIndexOf(".");
  if (dot < 0) {
    return EXTENSION_LANGUAGE[name.toLowerCase()] ?? "plaintext";
  }
  return EXTENSION_LANGUAGE[name.slice(dot + 1).toLowerCase()] ?? "plaintext";
}

/** Base name of a path. */
export function baseName(path: string): string {
  return path.split("/").filter(Boolean).pop() ?? path;
}

/** Compact id for tight UI (session/message ids can be long). */
export function shortId(id: string): string {
  return id.length > 14 ? `${id.slice(0, 14)}…` : id;
}

/** Human duration from epoch millis (e.g. "3s", "2m"). */
export function durationSince(startedAt: number, now: number = Date.now()): string {
  const seconds = Math.max(0, Math.round((now - startedAt) / 1000));
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  return `${hours}h`;
}

/** Pretty-print an unknown value for display without ever throwing. */
export function safeJson(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}
