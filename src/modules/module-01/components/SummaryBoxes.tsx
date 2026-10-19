import { useEffect, useState, type FC } from "react";
import type { SessionStats } from "../sessionStats";
import { formatRelativeDays, formatTimestamp } from "../sessionStats";

interface SummaryBoxesProps {
  projectName: string;
  projectDescription: string | null;
  /** ISO timestamp of when the project was created. */
  projectCreatedAt: string;
  stats: SessionStats;
}

interface SummaryTileProps {
  label: string;
  value: string;
  subtext: string;
}

const SummaryTile: FC<SummaryTileProps> = ({ label, value, subtext }) => (
  <div className="m1-summary-card">
    <span className="m1-summary-card__label">{label}</span>
    <span className="m1-summary-card__value m1-tabular" title={value}>
      {value}
    </span>
    <span className="m1-summary-card__subtext m1-tabular">{subtext}</span>
  </div>
);

export const SummaryBoxes: FC<SummaryBoxesProps> = ({
  projectName,
  projectDescription,
  projectCreatedAt,
  stats,
}) => {
  // Held in state (refreshed every minute) so "Hari ini" / "N hari yang lalu" stay correct
  // across midnight while render stays pure.
  const [nowMs, setNowMs] = useState(() => Date.now());
  useEffect(() => {
    const id = setInterval(() => setNowMs(Date.now()), 60_000);
    return () => clearInterval(id);
  }, []);
  const hasSessions = stats.totalSessions > 0;
  const imageCount = stats.totalImages;

  return (
    <>
      <div className="m1-summary-head">
        <h2 className="m1-summary-head__name">{projectName}</h2>
        <span className="m1-summary-head__desc">
          {projectDescription?.trim() ? projectDescription : "Belum ada deskripsi proyek"}
        </span>
      </div>

      <div className="m1-summary-grid" role="region" aria-label="Ringkasan data sesi">
        <SummaryTile
          label="Total sesi"
          value={String(stats.totalSessions)}
          subtext={
            hasSessions
              ? `${stats.activeCount} aktif · ${stats.archivedCount} diarsipkan`
              : "Belum ada sesi tercatat"
          }
        />
        <SummaryTile
          label="Total citra"
          value={imageCount === undefined ? "—" : String(imageCount)}
          subtext={
            imageCount === undefined
              ? "Jumlah citra belum tersedia"
              : `${imageCount} berkas citra terhubung`
          }
        />
        <SummaryTile
          label="Rentang waktu"
          value={hasSessions ? stats.durationLabel : "—"}
          subtext={hasSessions ? stats.periodLabel : "Belum ada sesi"}
        />
        <SummaryTile
          label="Aktivitas terakhir"
          value={stats.latestActivityDateLabel}
          subtext={
            stats.latestSession
              ? `${stats.latestActivityTimeLabel} · ${stats.latestSession.name}`
              : "Belum ada aktivitas"
          }
        />
        <SummaryTile
          label="Dibuat pada"
          value={formatTimestamp(projectCreatedAt)}
          subtext={formatRelativeDays(projectCreatedAt, nowMs)}
        />
      </div>
    </>
  );
};
