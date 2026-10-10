import Editor from "@monaco-editor/react";
import { languageForPath } from "../utils";

interface Props {
  path: string;
  value: string;
  theme: "dark" | "light";
  onChange?: (value: string) => void;
}

/**
 * In-memory editor. v1 exposes no file-write API, so changes stay local to the
 * running window (see IMPLEMENTATION_NOTES for the rationale).
 */
export function MonacoEditor({ path, value, theme, onChange }: Props) {
  return (
    <Editor
      theme={theme === "dark" ? "vs-dark" : "vs"}
      path={path}
      language={languageForPath(path)}
      value={value}
      onChange={(next) => onChange?.(next ?? "")}
      options={{
        minimap: { enabled: false },
        fontSize: 13,
        scrollBeyondLastLine: false,
        wordWrap: "on",
        automaticLayout: true,
        tabSize: 2,
      }}
    />
  );
}