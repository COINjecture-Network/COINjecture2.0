import { useState, useCallback } from 'react';
import { S3Service } from '@/lib/s3';
import type { S3Config, S3UploadParams } from '@/types/s3';

export const useS3 = (config: S3Config) => {
  const [isUploading, setIsUploading] = useState(false);
  const [uploadProgress, setUploadProgress] = useState(0);
  const [error, setError] = useState<Error | null>(null);

  const s3Service = new S3Service(config);

  const upload = useCallback(async (params: S3UploadParams) => {
    setIsUploading(true);
    setError(null);
    setUploadProgress(0);

    try {
      const result = await s3Service.upload(params);
      setUploadProgress(100);
      return result;
    } catch (err) {
      const error = err instanceof Error ? err : new Error('Upload failed');
      setError(error);
      throw error;
    } finally {
      setIsUploading(false);
    }
  }, [s3Service]);

  const download = useCallback(async (key: string) => {
    setError(null);
    try {
      return await s3Service.download(key);
    } catch (err) {
      const error = err instanceof Error ? err : new Error('Download failed');
      setError(error);
      throw error;
    }
  }, [s3Service]);

  const deleteFile = useCallback(async (key: string) => {
    setError(null);
    try {
      await s3Service.delete(key);
    } catch (err) {
      const error = err instanceof Error ? err : new Error('Delete failed');
      setError(error);
      throw error;
    }
  }, [s3Service]);

  const getPublicUrl = useCallback((key: string) => {
    return s3Service.getPublicUrl(key);
  }, [s3Service]);

  return {
    upload,
    download,
    deleteFile,
    getPublicUrl,
    isUploading,
    uploadProgress,
    error,
  };
};
