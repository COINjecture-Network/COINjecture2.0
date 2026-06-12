import { useEffect, useRef, useState } from "react";
import CodeMirror from "@uiw/react-codemirror";
import { javascript } from "@codemirror/lang-javascript";
import { json } from "@codemirror/lang-json";
import { oneDark } from "@codemirror/theme-one-dark";
import { cn } from "@/lib/utils";
import type { WorkspaceFilePath } from "./defaultSolverWorkspace";

type Props = {
  path: WorkspaceFilePath;
  value: string;
  onChange: (v: string) => void;
  dark: boolean;
  className?: string;
  minHeight?: string;
};

export function SolverCodeEditor({ path, value, onChange, dark, className, minHeight = "min(60vh, 520px)" }: Props) {
  const lang = path.endsWith(".json") ? json() : javascript({ jsx: false });
  const fillParent = minHeight === "100%";
  const containerRef = useRef<HTMLDivElement>(null);
  const [editorHeight, setEditorHeight] = useState(0);

  useEffect(() => {
    if (!fillParent) return;
    const el = containerRef.current;
    if (!el) return;

    const syncHeight = () => {
      const next = Math.floor(el.getBoundingClientRect().height);
      if (next > 0) setEditorHeight(next);
    };

    syncHeight();
    const observer = new ResizeObserver(syncHeight);
    observer.observe(el);
    return () => observer.disconnect();
  }, [fillParent]);

  const editor = (
    <CodeMirror
      value={value}
      height={fillParent ? `${editorHeight}px` : minHeight}
      theme={dark ? oneDark : undefined}
      extensions={[lang]}
      onChange={onChange}
      className={cn("text-sm", !fillParent && "overflow-hidden border border-border/40 rounded-md", className)}
      basicSetup={{
        lineNumbers: true,
        foldGutter: true,
        bracketMatching: true,
        closeBrackets: true,
        indentOnInput: true,
      }}
    />
  );

  if (fillParent) {
    return (
      <div
        ref={containerRef}
        className={cn(
          "h-0 min-h-0 min-w-0 flex-1 overflow-hidden rounded-md border border-border/40",
          "[&_.cm-editor]:h-full [&_.cm-scroller]:overflow-auto",
          className,
        )}
      >
        {editorHeight > 0 ? editor : null}
      </div>
    );
  }

  return editor;
}
