/// <reference lib="webworker" />
import { mineHeaderPowSync } from "./mining-pow-core";
import type { Block } from "./rpc-client";

type InMsg = { header: Block["header"]; difficulty: number };

self.onmessage = (e: MessageEvent<InMsg>) => {
  try {
    const { header, difficulty } = e.data;
    let last = 0;
    const result = mineHeaderPowSync(header, difficulty, (nonce, hash) => {
      // Post often enough that the Solver Lab banner updates within ~1s on typical hardware.
      if (nonce - last >= 20_000) {
        last = nonce;
        self.postMessage({ type: "progress", nonce, hash });
      }
    });
    self.postMessage({ type: "done", result });
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    self.postMessage({ type: "error", message });
  }
};
