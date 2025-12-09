export interface S3Config {
  accessKeyId: string;
  secretAccessKey: string;
  region: string;
  bucket: string;
}

export interface S3UploadParams {
  file: File;
  key: string;
  contentType?: string;
}

export interface S3UploadResult {
  key: string;
  url: string;
  bucket: string;
}
