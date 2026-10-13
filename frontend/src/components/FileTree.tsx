import { useEffect, useState } from "react";
import { listFiles } from "../commands";
import type { FileNode } from "../types";
import { describeError } from "../utils";

interface Props {
  onOpenFile: (path: string) => void;
  activePath: string | null;
}

/** Two-pane file browser; directories expand lazily (one request each). */
export function FileTree({ onOpenFile, activePath }: Props) {
  const [roots, setRoots] = useState<FileNode[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const nodes = await listFiles();
        if (!cancelled) setRoots(nodes);
      } catch (cause) {
        if (!cancelled) setError(describeError(cause));
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div className="filetree">
      {error && (
        <p className="filetree__error" role="alert">
          {error}
        </p>
      )}
      {loading && roots.length === 0 && !error && (
        <p className="filetree__muted">Loading files…</p>
      )}
      {!loading && !error && roots.length === 0 && (
        <p className="filetree__muted">No files to show for this project.</p>
      )}
      {roots.map((node) => (
        <TreeNode
          key={node.path}
          node={node}
          depth={0}
          onOpenFile={onOpenFile}
          activePath={activePath}
        />
      ))}
    </div>
  );
}

interface TreeNodeProps {
  node: FileNode;
  depth: number;
  onOpenFile: (path: string) => void;
  activePath: string | null;
}

function TreeNode({ node, depth, onOpenFile, activePath }: TreeNodeProps) {
  const [expanded, setExpanded] = useState(false);
  const [children, setChildren] = useState<FileNode[] | null>(null);
  const [loading, setLoading] = useState(false);

  const isDirectory = node.type === "directory";
  const indent = 8 + depth * 14;

  async function toggle(): Promise<void> {
    if (!isDirectory) {
      onOpenFile(node.path);
      return;
    }
    if (!expanded && children === null) {
      setLoading(true);
      try {
        setChildren(await listFiles(node.path));
      } catch {
        setChildren([]);
      } finally {
        setLoading(false);
      }
    }
    setExpanded((value) => !value);
  }

  return (
    <>
      <button
        type="button"
        className={`tnode${activePath === node.path ? " tnode--active" : ""}${
          node.ignored ? " tnode--ignored" : ""
        }`}
        style={{ paddingLeft: indent }}
        onClick={() => void toggle()}
        title={node.path}
        aria-expanded={isDirectory ? expanded : undefined}
      >
        <span className="tnode__icon" aria-hidden>
          {isDirectory ? (expanded ? "▾" : "▸") : "•"}
        </span>
        <span className="tnode__name">{node.name}</span>
      </button>
      {isDirectory && expanded && (
        <div>
          {loading && (
            <div className="tnode__loading" style={{ paddingLeft: indent + 14 }}>
              …
            </div>
          )}
          {children?.map((child) => (
            <TreeNode
              key={child.path}
              node={child}
              depth={depth + 1}
              onOpenFile={onOpenFile}
              activePath={activePath}
            />
          ))}
        </div>
      )}
    </>
  );
}