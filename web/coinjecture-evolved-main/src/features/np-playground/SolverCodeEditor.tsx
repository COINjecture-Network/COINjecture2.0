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

  const editor = (
    <CodeMirror
      value={value}
      height={fillParent ? "100%" : minHeight}
      theme={dark ? oneDark : undefined}
      extensions={[lang]}
      onChange={onChange}
      className={cn("text-sm overflow-hidden", !fillParent && "border border-border/40 rounded-md", className)}
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
      <div className={cn("flex h-full min-h-[200px] min-w-0 flex-1 flex-col border border-border/40 rounded-md overflow-hidden", className)}>
        {editor}
      </div>
    );
  }

  return editor;
}
