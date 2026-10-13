/**
 * Hand-written inline SVG iconography for the shell.
 *
 * The file explorer and the tab strip both need to say "what kind of thing is
 * this" at a glance, so the glyphs live here instead of being duplicated (or
 * pulling in an icon dependency the bundle does not need).
 */

/** Accent for names whose extension we do not recognise. */
const FALLBACK_TINT = "var(--muted)";

/**
 * Extension to accent colour. Roughly follows the associations editors already
 * taught our eyes, and stays legible against both the dark and light surfaces.
 */
const EXTENSION_TINTS: Record<string, string> = {
  ts: "#3178c6",
  tsx: "#3178c6",
  mts: "#3178c6",
  cts: "#3178c6",
  js: "#e8c547",
  jsx: "#e8c547",
  mjs: "#e8c547",
  cjs: "#e8c547",
  json: "#c9b458",
  jsonc: "#c9b458",
  css: "#56a3d9",
  scss: "#d4739d",
  sass: "#d4739d",
  less: "#6b8fd6",
  html: "#e8734a",
  htm: "#e8734a",
  vue: "#4fbf8b",
  svelte: "#e8734a",
  md: "#7aa2f7",
  mdx: "#7aa2f7",
  rst: "#7aa2f7",
  txt: "#9aa7b8",
  rs: "#dea584",
  go: "#39c6d6",
  py: "#5aa7d6",
  rb: "#e05a6b",
  java: "#e07b53",
  kt: "#a97bd6",
  swift: "#f08a4b",
  c: "#7c9ed9",
  h: "#7c9ed9",
  cpp: "#7c9ed9",
  hpp: "#7c9ed9",
  cc: "#7c9ed9",
  cs: "#7ac36a",
  php: "#8f9bd6",
  sh: "#89e051",
  bash: "#89e051",
  zsh: "#89e051",
  fish: "#89e051",
  toml: "#c98b6b",
  yml: "#e06c75",
  yaml: "#e06c75",
  ini: "#9aa7b8",
  env: "#e8c547",
  sql: "#e0a458",
  graphql: "#e36bb0",
  gql: "#e36bb0",
  svg: "#e0a458",
  png: "#a97bd6",
  jpg: "#a97bd6",
  jpeg: "#a97bd6",
  gif: "#a97bd6",
  webp: "#a97bd6",
  ico: "#a97bd6",
  lock: "#8b95a5",
  wasm: "#8f7bd6",
  proto: "#7aa2f7",
};

/** Names that carry the meaning without a useful extension. */
const NAME_TINTS: Record<string, string> = {
  dockerfile: "#56a3d9",
  "docker-compose.yml": "#56a3d9",
  makefile: "#c98b6b",
  "cmakelists.txt": "#7c9ed9",
  ".gitignore": "#e06c75",
  ".gitattributes": "#e06c75",
  ".dockerignore": "#56a3d9",
  ".editorconfig": "#9aa7b8",
  ".env": "#e8c547",
  ".npmrc": "#e06c75",
  "cargo.lock": "#dea584",
  "cargo.toml": "#c98b6b",
  "package-lock.json": "#c9b458",
  license: "#e0a458",
  readme: "#7aa2f7",
};

function extensionOf(name: string): string {
  const dot = name.lastIndexOf(".");
  if (dot <= 0) return "";
  return name.slice(dot + 1);
}

/** Accent colour for a file name (a bare name or a full path both work). */
export function fileTint(name: string): string {
  const lower = name.toLowerCase();
  const base = lower.slice(lower.lastIndexOf("/") + 1);
  const named = NAME_TINTS[base];
  if (named) return named;
  return EXTENSION_TINTS[extensionOf(base)] ?? FALLBACK_TINT;
}

interface SizedProps {
  size?: number;
}

interface FileIconProps extends SizedProps {
  name: string;
}

/** A page glyph tinted by the file's extension. */
export function FileIcon({ name, size = 16 }: FileIconProps) {
  const tint = fileTint(name);
  return (
    <svg
      className="fileicon"
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      aria-hidden
      focusable="false"
    >
      <path
        d="M4.5 1.75h4.6l3.65 3.65v8.35a.75.75 0 0 1-.75.75h-7.5a.75.75 0 0 1-.75-.75V2.5a.75.75 0 0 1 .75-.75Z"
        stroke={tint}
        strokeWidth="1.2"
        strokeLinejoin="round"
      />
      <path
        d="M9 1.9v3.1a.6.6 0 0 0 .6.6h3.05"
        stroke={tint}
        strokeWidth="1.2"
        strokeLinejoin="round"
        opacity="0.6"
      />
      <path
        d="M5.4 9.3h5.2"
        stroke={tint}
        strokeWidth="1.2"
        strokeLinecap="round"
        opacity="0.5"
      />
    </svg>
  );
}

interface FolderIconProps extends SizedProps {
  open?: boolean;
}

/** Folder glyph; `open` switches between the closed and the opened silhouette. */
export function FolderIcon({ open = false, size = 16 }: FolderIconProps) {
  const tint = open ? "var(--accent)" : "var(--muted)";
  return (
    <svg
      className="fileicon"
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      aria-hidden
      focusable="false"
    >
      <path
        d="M1.75 3.9a.9.9 0 0 1 .9-.9h3.1a.9.9 0 0 1 .68.31l.72.83h6.7a.9.9 0 0 1 .9.9v6.06a.9.9 0 0 1-.9.9H2.65a.9.9 0 0 1-.9-.9V3.9Z"
        stroke={tint}
        strokeWidth="1.2"
        strokeLinejoin="round"
        fill={open ? tint : "none"}
        fillOpacity={open ? 0.22 : undefined}
      />
      {open && (
        <path
          d="M2.2 7.1h11.6"
          stroke={tint}
          strokeWidth="1.2"
          strokeLinecap="round"
          opacity="0.65"
        />
      )}
    </svg>
  );
}

/** A terminal window, used to mark an agent session tab. */
export function TerminalIcon({ size = 14 }: SizedProps) {
  return (
    <svg
      className="fileicon"
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      aria-hidden
      focusable="false"
    >
      <rect
        x="1.6"
        y="2.6"
        width="12.8"
        height="10.8"
        rx="2"
        stroke="var(--accent)"
        strokeWidth="1.2"
      />
      <path
        d="m4.5 6.4 2.1 1.8-2.1 1.8"
        stroke="var(--accent)"
        strokeWidth="1.2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path
        d="M8.2 10.3h3.3"
        stroke="var(--accent)"
        strokeWidth="1.2"
        strokeLinecap="round"
      />
    </svg>
  );
}

/** A four-point spark, used to mark a skill tab. */
export function SkillIcon({ size = 14 }: SizedProps) {
  return (
    <svg
      className="fileicon"
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      aria-hidden
      focusable="false"
    >
      <path
        d="M8 1.8 9.4 6.6 14.2 8 9.4 9.4 8 14.2 6.6 9.4 1.8 8 6.6 6.6 8 1.8Z"
        stroke="var(--warning)"
        strokeWidth="1.2"
        strokeLinejoin="round"
      />
    </svg>
  );
}

/**
 * A split diff glyph (plus on one side, minus on the other), used to mark a
 * working-tree diff tab.
 */
export function DiffIcon({ size = 14 }: SizedProps) {
  return (
    <svg
      className="fileicon"
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      aria-hidden
      focusable="false"
    >
      <rect
        x="1.6"
        y="2.6"
        width="12.8"
        height="10.8"
        rx="2"
        stroke="var(--secondary)"
        strokeWidth="1.2"
      />
      <path
        d="M8 2.6v10.8"
        stroke="var(--secondary)"
        strokeWidth="1.2"
        opacity="0.7"
      />
      <path
        d="M4.7 8.5h2.6"
        stroke="var(--secondary)"
        strokeWidth="1.2"
        strokeLinecap="round"
      />
      <path
        d="M11.3 7v3M9.8 8.5h3"
        stroke="var(--secondary)"
        strokeWidth="1.2"
        strokeLinecap="round"
      />
    </svg>
  );
}
