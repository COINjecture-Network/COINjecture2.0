/// <reference lib="webworker" />
import type { ProblemType } from "@/lib/rpc-client";
import { buildRunner } from "./solverBuild";

export type WorkerIn = { files: Record<string, string>; problem: ProblemType };

self.onmessage = (e: MessageEvent<WorkerIn>) => {
  const { files, problem } = e.data;
  try {
    const run = buildRunner(files);
    const t0 = performance.now();
    const solution = run(problem);
    self.postMessage({ ok: true, solution, timeMs: performance.now() - t0 });
  } catch (err) {
    self.postMessage({
      ok: false,
      error: err instanceof Error ? err.message : String(err),
    });
  }
};
