import type { ProblemType } from "@/lib/rpc-client";
import type { WorkspaceFilePath } from "./defaultSolverWorkspace";
import SolverWorker from "./solverRunner.worker?worker";

export type RunUserSolverResult =
  | { ok: true; solution: unknown; timeMs: number }
  | { ok: false; error: string; timeMs?: number };

export { buildRunner } from "./solverBuild";

export function runUserSolver(
  files: Record<WorkspaceFilePath, string>,
  problem: ProblemType,
  timeoutMs = 45000
): Promise<RunUserSolverResult> {
  return new Promise((resolve) => {
    const worker = new SolverWorker();
    const timer = window.setTimeout(() => {
      worker.terminate();
      resolve({ ok: false, error: `Solver stopped after ${timeoutMs}ms (infinite loop or too slow).` });
    }, timeoutMs);

    worker.onmessage = (e: MessageEvent<RunUserSolverResult>) => {
      window.clearTimeout(timer);
      worker.terminate();
      resolve(e.data);
    };

    worker.onerror = (ev) => {
      window.clearTimeout(timer);
      worker.terminate();
      resolve({ ok: false, error: ev.message || "Worker error" });
    };

    worker.postMessage({ files, problem });
  });
}
