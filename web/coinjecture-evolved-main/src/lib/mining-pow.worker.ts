/// <reference lib="webworker" />
import { mineHeaderPowSync } from "./mining-pow-core";
import type { Block } from "./rpc-client";

type InMsg = { header: Block["header"]; difficulty: number };

self.onmessage = (e: MessageEvent<InMsg>) => {
  const { header, difficulty } = e.data;
  let last = 0;
  const result = mineHeaderPowSync(header, difficulty, (nonce, hash) => {
    if (nonce - last >= 125_000) {
      last = nonce;
      self.postMessage({ type: "progress", nonce, hash });
    }
  });
  self.postMessage({ type: "done", result });
};
