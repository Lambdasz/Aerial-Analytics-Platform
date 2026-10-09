import type { FC } from "react";
import type { SessionStats } from "../sessionStats";

interface SummaryBoxesProps {
  stats: SessionStats;
  onFocusLatestSession: (sessionId: string) => void;
}

export const SummaryBoxes: FC<SummaryBoxesProps> = ({ stats, onFocusLatestSession }) => {
  const activePercent =
    stats.totalSessions > 0 ? (stats.activeCount / stats.totalSessions) * 100 : 0;
  const archivedPercent =
    stats.totalSessions > 0 ? (stats.archivedCount / stats.totalSessions) * 100 : 0;

  return (
    <div className="m1-summary-grid" role="region" aria-label="Ringkasan data sesi">
      {/* 1. Sesi (Pastel Blue) */}
      <div className="m1-summary-card m1-summary-card--blue">
        <div className="m1-summary-card__header">
          <span className="m1-summary-card__label">Total Sesi</span>
        </div>
        <div className="m1-summary-card__value m1-tabular m1-condensed">{stats.totalSessions}</div>
        <div
          className="m1-segment-track"
          aria-hidden="true"
          title={`${stats.activeCount} aktif, ${stats.archivedCount} diarsipkan`}
        >
          <div className="m1-segment-fill--active" style={{ width: `${activePercent}%` }} />
          <div className="m1-segment-fill--archived" style={{ width: `${archivedPercent}%` }} />
        </div>
        <p className="m1-summary-card__subtext m1-tabular">
          {stats.totalSessions === 0
            ? "Belum ada sesi tercatat"
            : `${stats.activeCount} aktif | ${stats.archivedCount} diarsipkan`}
        </p>
      </div>

      {/* 2. Gambar (Pastel Purple) */}
      <div className="m1-summary-card m1-summary-card--purple">
        <div className="m1-summary-card__header">
          <span className="m1-summary-card__label">Total Gambar</span>
        </div>
        <div className="m1-summary-card__value m1-tabular m1-condensed">
          {stats.totalImages !== undefined ? stats.totalImages : "—"}
        </div>
        <p className="m1-summary-card__subtext">
          {stats.totalImages !== undefined
            ? `${stats.totalImages} berkas foto terhubung`
            : "Belum ada data gambar"}
        </p>
      </div>

      {/* 3. Periode Data (Pastel Green) */}
      <div className="m1-summary-card m1-summary-card--green">
        <div className="m1-summary-card__header">
          <span className="m1-summary-card__label">Rentang Waktu</span>
        </div>
        <div className="m1-summary-card__value m1-summary-card__value--medium m1-tabular m1-condensed">
          {stats.totalSessions > 0 ? stats.durationLabel : "—"}
        </div>
        <p className="m1-summary-card__subtext m1-tabular">
          {stats.totalSessions > 0 ? stats.periodLabel : "Belum ada rentang waktu"}
        </p>
      </div>

      {/* 4. Sesi Terakhir (Pastel Rose) */}
      <div
        className={`m1-summary-card m1-summary-card--rose ${stats.latestSession ? "m1-summary-card--interactive" : ""}`}
        onClick={() => {
          if (stats.latestSession) {
            onFocusLatestSession(stats.latestSession.id);
          }
        }}
        onKeyDown={(e) => {
          if ((e.key === "Enter" || e.key === " ") && stats.latestSession) {
            e.preventDefault();
            onFocusLatestSession(stats.latestSession.id);
          }
        }}
        tabIndex={stats.latestSession ? 0 : undefined}
        role={stats.latestSession ? "button" : undefined}
        aria-label={
          stats.latestSession ? `Sorot sesi terakhir: ${stats.latestSession.name}` : undefined
        }
      >
        <div className="m1-summary-card__header">
          <span className="m1-summary-card__label">Aktivitas Terakhir</span>
        </div>
        <div
          className="m1-summary-card__value m1-summary-card__value--text"
          title={stats.latestSession?.name}
        >
          {stats.latestSession ? stats.latestSession.name : "—"}
        </div>
        <p className="m1-summary-card__subtext m1-tabular">
          {stats.latestSession
            ? `${stats.latestSessionDateLabel} · klik untuk sorot`
            : "Belum ada aktivitas"}
        </p>
      </div>
    </div>
  );
};
