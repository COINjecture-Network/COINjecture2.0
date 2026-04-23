import type { ProblemType } from "@/lib/rpc-client";
import type { WorkspaceFilePath } from "./defaultSolverWorkspace";
import {
  createUserSolverWorker,
  type UserSolverWorkerIn,
} from "./solverBuild";

export type RunUserSolverResult =
  | { ok: true; solution: unknown; timeMs: number }
  | { ok: false; error: string; timeMs?: number };

export { buildUserSolverWorkerScript, createUserSolverWorker } from "./solverBuild";

export function runUserSolver(
  files: Record<WorkspaceFilePath, string>,
  problem: ProblemType,
  timeoutMs = 45000
): Promise<RunUserSolverResult> {
  return new Promise((resolve) => {
    const { worker, revokeObjectUrl } = createUserSolverWorker(files);
    const payload: UserSolverWorkerIn = { problem };

    const timer = window.setTimeout(() => {
      worker.terminate();
      revokeObjectUrl();
      resolve({
        ok: false,
        error: `Solver stopped after ${timeoutMs}ms (infinite loop or too slow).`,
      });
    }, timeoutMs);

    worker.onmessage = (e: MessageEvent<RunUserSolverResult>) => {
      window.clearTimeout(timer);
      worker.terminate();
      revokeObjectUrl();
      resolve(e.data);
    };

    worker.onerror = (ev) => {
      window.clearTimeout(timer);
      worker.terminate();
      revokeObjectUrl();
      resolve({ ok: false, error: ev.message || "Worker error" });
    };

    worker.postMessage(payload);
  });
}
