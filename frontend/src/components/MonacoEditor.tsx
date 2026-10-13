import Editor from "@monaco-editor/react";
import { useEffect, useRef } from "react";
import { languageForPath } from "../utils";

interface Props {
  path: string;
  value: string;
  theme: "dark" | "light";
  onChange?: (value: string) => void;
  /** Invoked on Ctrl/Cmd+S; when omitted no keybinding is registered. */
  onSave?: () => void;
}

/**
 * In-memory editor. Text changes stay local to the running window until the
 * owner persists them: `onSave` (wired to Ctrl/Cmd+S) lets a file tab write
 * through `write_file`, while a skill tab omits it and keeps its own save path.
 */
export function MonacoEditor({ path, value, theme, onChange, onSave }: Props) {
  // Monaco registers a keybinding once at mount, but `onSave` changes identity
  // between edits; route the command through a ref so it always calls the
  // latest callback (never a stale closure over earlier content).
  const onSaveRef = useRef(onSave);
  useEffect(() => {
    onSaveRef.current = onSave;
  }, [onSave]);

  return (
    <Editor
      theme={theme === "dark" ? "vs-dark" : "vs"}
      path={path}
      language={languageForPath(path)}
      value={value}
      onChange={(next) => onChange?.(next ?? "")}
      onMount={(editor, monaco) => {
        if (!onSaveRef.current) return;
        editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => {
          onSaveRef.current?.();
        });
      }}
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
