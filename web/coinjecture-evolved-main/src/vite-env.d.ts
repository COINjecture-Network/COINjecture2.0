/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_HF_DATASET_URL?: string;
}

declare module "*?worker" {
  const workerConstructor: new () => Worker;
  export default workerConstructor;
}
