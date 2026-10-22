import React from "react";
import { Button } from "@blueprintjs/core";

export interface AnnotationFormProps {
  annotationText: string;
  latitude: string;
  longitude: string;
  onTextChange: (text: string) => void;
  onLatitudeChange: (lat: string) => void;
  onLongitudeChange: (lng: string) => void;
  onSubmit: () => void;
}

export const AnnotationForm: React.FC<AnnotationFormProps> = ({
  annotationText,
  latitude,
  longitude,
  onTextChange,
  onLatitudeChange,
  onLongitudeChange,
  onSubmit,
}) => {
  return (
    <div
      style={{
        marginTop: "20px",
        paddingTop: "16px",
        borderTop: "1px solid var(--color-hairline-silver)",
      }}
    >
      <div
        style={{
          marginBottom: "10px",
          background: "var(--color-gallery-white)",
          boxShadow: "var(--shadow-subtle)",
          borderRadius: "12px",
          padding: "8px 12px",
        }}
      >
        <textarea
          placeholder="Tulis catatan annotation..."
          value={annotationText}
          onChange={(event) => onTextChange(event.target.value)}
          style={{
            width: "100%",
            minHeight: "60px",
            border: "none",
            outline: "none",
            background: "transparent",
            color: "var(--color-ink)",
            fontSize: "14px",
            fontFamily: "var(--font-sf-pro-text)",
            resize: "vertical",
          }}
        />
      </div>

      <div
        style={{
          display: "flex",
          gap: "8px",
          marginBottom: "12px",
        }}
      >
        <div
          style={{
            flex: 1,
            display: "flex",
            alignItems: "center",
            background: "var(--color-gallery-white)",
            boxShadow: "var(--shadow-subtle)",
            borderRadius: "12px",
            padding: "8px 12px",
          }}
        >
          <input
            type="number"
            placeholder="Latitude"
            value={latitude}
            onChange={(event) => onLatitudeChange(event.target.value)}
            style={{
              width: "100%",
              border: "none",
              outline: "none",
              background: "transparent",
              color: "var(--color-ink)",
              fontSize: "14px",
              fontFamily: "var(--font-sf-pro-text)",
            }}
          />
        </div>

        <div
          style={{
            flex: 1,
            display: "flex",
            alignItems: "center",
            background: "var(--color-gallery-white)",
            boxShadow: "var(--shadow-subtle)",
            borderRadius: "12px",
            padding: "8px 12px",
          }}
        >
          <input
            type="number"
            placeholder="Longitude"
            value={longitude}
            onChange={(event) => onLongitudeChange(event.target.value)}
            style={{
              width: "100%",
              border: "none",
              outline: "none",
              background: "transparent",
              color: "var(--color-ink)",
              fontSize: "14px",
              fontFamily: "var(--font-sf-pro-text)",
            }}
          />
        </div>
      </div>

      <Button
        icon="add"
        text="Buat Annotation"
        fill
        onClick={onSubmit}
        style={{
          borderRadius: "999px",
          background: "var(--color-apple-blue)",
          color: "#fff",
          boxShadow: "none",
          fontWeight: 600,
          padding: "8px 16px",
        }}
      />
    </div>
  );
};
