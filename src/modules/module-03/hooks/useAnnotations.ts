import { useCallback, useEffect, useState } from "react";
import type { AnnotationFeature, SpatialGeometry } from "../types/layer";
import {
  createAnnotation,
  deleteAnnotation,
  getSavedAnnotations,
  updateAnnotation,
} from "../services/api";

export interface UseAnnotationsResult {
  annotations: AnnotationFeature[];
  isLoading: boolean;
  error: string | null;
  fetchAnnotations: () => Promise<void>;
  addAnnotation: (args: {
    text: string;
    geometry: SpatialGeometry;
    id?: string;
  }) => Promise<AnnotationFeature>;
  editAnnotation: (annotation: AnnotationFeature) => Promise<AnnotationFeature>;
  removeAnnotation: (id: string) => Promise<boolean>;
}

/**
 * Hook kustom untuk pengelolaan catatan anotasi inspeksi spasial (CRUD).
 * Terhubung langsung ke penyimpanan disk atomik backend via Tauri IPC.
 */
export function useAnnotations(): UseAnnotationsResult {
  const [annotations, setAnnotations] = useState<AnnotationFeature[]>([]);
  const [isLoading, setIsLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);

  const fetchAnnotations = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      const data = await getSavedAnnotations();
      setAnnotations(data || []);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg);
    } finally {
      setIsLoading(false);
    }
  }, []);

  const addAnnotation = useCallback(
    async (args: {
      text: string;
      geometry: SpatialGeometry;
      id?: string;
    }): Promise<AnnotationFeature> => {
      const id = args.id || crypto.randomUUID();
      const createdAt = new Date().toISOString();
      const created = await createAnnotation({
        id,
        annotationId: id,
        text: args.text,
        geometry: args.geometry,
        createdAt,
      });

      setAnnotations((prev) => [created, ...prev]);
      return created;
    },
    [],
  );

  const editAnnotation = useCallback(
    async (annotation: AnnotationFeature): Promise<AnnotationFeature> => {
      const updated = await updateAnnotation(annotation);
      setAnnotations((prev) =>
        prev.map((item) =>
          (item.id || item.annotation_id) === (updated.id || updated.annotation_id)
            ? updated
            : item,
        ),
      );
      return updated;
    },
    [],
  );

  const removeAnnotation = useCallback(async (id: string): Promise<boolean> => {
    const success = await deleteAnnotation(id);
    if (success) {
      setAnnotations((prev) => prev.filter((item) => item.id !== id && item.annotation_id !== id));
    }
    return success;
  }, []);

  useEffect(() => {
    let cancelled = false;

    getSavedAnnotations()
      .then((data) => {
        if (!cancelled) {
          setAnnotations(data || []);
          setIsLoading(false);
        }
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : String(err));
          setIsLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  return {
    annotations,
    isLoading,
    error,
    fetchAnnotations,
    addAnnotation,
    editAnnotation,
    removeAnnotation,
  };
}
