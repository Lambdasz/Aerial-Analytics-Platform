import React, { useEffect, useState, useCallback, useRef } from "react";
import {
  Dialog,
  DialogBody,
  DialogFooter,
  Button,
  ProgressBar,
  Intent,
  Callout,
} from "@blueprintjs/core";
import { cancelImportJob, getImportJobStatus } from "../services/api";
import type { JobRecord } from "../types/storage";

export interface ImportProgressModalProps {
  isOpen: boolean;
  jobId: string | null;
  onClose: () => void;
  onSuccess: (resultImageId: string) => void;
}

export const ImportProgressModal: React.FC<ImportProgressModalProps> = ({
  isOpen,
  jobId,
  onClose,
  onSuccess,
}) => {
  const [jobRecord, setJobRecord] = useState<JobRecord | null>(null);
  const [isCancelling, setIsCancelling] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const pollTimerRef = useRef<number | null>(null);

  // Cek apakah proses sudah memasuki Point of No Return
  const isPointOfNoReturn = jobRecord?.stage === "registrasi" || jobRecord?.stage === "selesai";

  const fetchStatus = useCallback(async () => {
    if (!jobId) return;
    try {
      const record = await getImportJobStatus(jobId);
      setJobRecord(record);

      if (record.status === "completed") {
        if (pollTimerRef.current) clearInterval(pollTimerRef.current);
        if (record.result_image_id) {
          onSuccess(record.result_image_id);
        }
      } else if (record.status === "failed") {
        if (pollTimerRef.current) clearInterval(pollTimerRef.current);
        setErrorMsg(record.message || "Proses impor citra gagal.");
      } else if (record.status === "cancelled") {
        if (pollTimerRef.current) clearInterval(pollTimerRef.current);
        setErrorMsg("Proses impor telah dibatalkan.");
      }
    } catch (err: unknown) {
      console.warn("Gagal polling status job:", err);
    }
  }, [jobId, onSuccess]);

  useEffect(() => {
    if (!isOpen || !jobId) {
      if (pollTimerRef.current) clearInterval(pollTimerRef.current);
      return;
    }

    // Ambil status di turn asynchronous berikutnya agar tidak cascading render
    const initialTimer = window.setTimeout(() => {
      void fetchStatus();
    }, 10);

    // Polling periodik (setiap 600ms) untuk sinkronisasi progres
    pollTimerRef.current = window.setInterval(() => {
      void fetchStatus();
    }, 600);

    return () => {
      window.clearTimeout(initialTimer);
      if (pollTimerRef.current) {
        clearInterval(pollTimerRef.current);
      }
    };
  }, [isOpen, jobId, fetchStatus]);

  const handleCancel = async () => {
    if (!jobId || isCancelling || isPointOfNoReturn) return;
    setIsCancelling(true);
    try {
      await cancelImportJob(jobId);
      await fetchStatus();
    } catch (err: unknown) {
      console.error("Gagal membatalkan job:", err);
    } finally {
      setIsCancelling(false);
    }
  };

  const stageDisplayLabel = (stage: string) => {
    switch (stage) {
      case "validasi":
        return "1. Validasi Berkas & Bounding Box";
      case "salin_asli":
        return "2. Menyalin Berkas Asli ke Penyimpanan";
      case "dekompresi":
        return "3. Dekompresi IDCT & Resampling Citra";
      case "loop_adaptif":
        return "4. Optimasi Kompresi Pratinjau (≤ 5 MB)";
      case "registrasi":
        return "5. Pendaftaran Layer & Penulisan Metadata";
      case "selesai":
        return "Selesai";
      default:
        return stage;
    }
  };

  const progressVal = Math.min(Math.max((jobRecord?.progress_percent ?? 0) / 100, 0), 1);
  const isFinished =
    jobRecord?.status === "completed" ||
    jobRecord?.status === "failed" ||
    jobRecord?.status === "cancelled";

  return (
    <Dialog
      isOpen={isOpen}
      onClose={() => {
        onClose();
      }}
      title="Proses Impor Citra Orthomosaic"
      icon="cloud-upload"
      style={{ width: "480px" }}
      canOutsideClickClose={isFinished}
    >
      <DialogBody>
        <div style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
          <div>
            <div style={{ display: "flex", justifyContent: "space-between", marginBottom: "6px" }}>
              <span style={{ fontWeight: 600, fontSize: "13px" }}>
                {jobRecord ? stageDisplayLabel(jobRecord.stage) : "Menyiapkan pekerjaan..."}
              </span>
              <span style={{ fontWeight: 600, fontSize: "13px", color: "#5c7080" }}>
                {Math.round(progressVal * 100)}%
              </span>
            </div>
            <ProgressBar
              value={progressVal}
              intent={
                jobRecord?.status === "failed"
                  ? Intent.DANGER
                  : jobRecord?.status === "completed"
                    ? Intent.SUCCESS
                    : Intent.PRIMARY
              }
              stripes={jobRecord?.status === "running"}
              animate={jobRecord?.status === "running"}
            />
          </div>

          <div style={{ fontSize: "12px", color: "#5c7080" }}>
            {jobRecord?.message || "Menghubungkan ke backend pengolah citra..."}
          </div>

          {isPointOfNoReturn && jobRecord?.status === "running" && (
            <Callout intent={Intent.WARNING} icon="lock" title="Tahap Akhir Tidak Dapat Dibatalkan">
              Proses telah memasuki tahap akhir (registrasi layer dan rename atomik). Pembatalan
              tidak dapat dilakukan lagi untuk mencegah kerusakan integritas data.
            </Callout>
          )}

          {errorMsg && (
            <Callout intent={Intent.DANGER} icon="error" title="Impor Gagal">
              {errorMsg}
            </Callout>
          )}

          {jobRecord?.status === "running" && (
            <div style={{ fontSize: "11px", color: "#8a9ba8", fontStyle: "italic" }}>
              Tips: Anda dapat menutup panel ini. Pekerjaan akan terus berjalan di latar belakang
              dan Anda dapat membukanya kembali sewaktu-waktu.
            </div>
          )}
        </div>
      </DialogBody>

      <DialogFooter
        actions={
          <div style={{ display: "flex", gap: "8px" }}>
            {jobRecord?.status === "running" && (
              <Button
                intent={Intent.DANGER}
                outlined
                icon="cross"
                text={isCancelling ? "Membatalkan..." : "Batalkan Impor"}
                disabled={isCancelling || isPointOfNoReturn}
                onClick={() => void handleCancel()}
              />
            )}
            <Button
              text={isFinished ? "Tutup" : "Lanjutkan di Latar Belakang"}
              intent={isFinished ? Intent.PRIMARY : Intent.NONE}
              onClick={onClose}
            />
          </div>
        }
      />
    </Dialog>
  );
};
