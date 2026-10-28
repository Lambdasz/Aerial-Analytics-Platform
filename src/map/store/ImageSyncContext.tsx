/* eslint-disable react-refresh/only-export-components */
import React, { createContext, useContext, useState, useCallback } from "react";

type InteractionSource = "map" | "gallery" | null;

interface ImageSyncState {
  activeImageId: string | null;
  interactionSource: InteractionSource;
  setActiveImage: (id: string | null, source: InteractionSource) => void;
}

const ImageSyncContext = createContext<ImageSyncState | undefined>(undefined);

export const ImageSyncProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [activeImageId, setActiveImageId] = useState<string | null>(null);
  const [interactionSource, setInteractionSource] = useState<InteractionSource>(null);

  const setActiveImage = useCallback((id: string | null, source: InteractionSource) => {
    setActiveImageId(id);
    setInteractionSource(source);
  }, []);

  return (
    <ImageSyncContext.Provider value={{ activeImageId, interactionSource, setActiveImage }}>
      {children}
    </ImageSyncContext.Provider>
  );
};

export const useImageSync = () => {
  const context = useContext(ImageSyncContext);
  if (context === undefined) {
    throw new Error("useImageSync must be used within an ImageSyncProvider");
  }
  return context;
};
