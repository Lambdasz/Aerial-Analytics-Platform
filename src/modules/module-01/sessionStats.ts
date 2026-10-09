import type { Session } from "./sessionCommands";

export interface SessionStats {
  totalSessions: number;
  activeCount: number;
  archivedCount: number;
  totalImages?: number;
  overallMinEpoch: number;
  overallMaxEpoch: number;
  periodLabel: string;
  durationLabel: string;
  latestSession: Session | null;
  latestSessionDateLabel: string;
}

export interface TimelinePosition {
  leftPercent: number;
  widthPercent: number;
}

const UUID_REGEX = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/** Validate UUID string format on client-side before dispatching IPC command */
export function isValidUuid(val: string): boolean {
  return UUID_REGEX.test(val.trim());
}

/** Get short 8-character ID */
export function shortId(id: string): string {
  return id.trim().slice(0, 8);
}

/**
 * Parse camera local ISO timestamp without timezone shifts.
 * SQLite/Rust stores camera local time, potentially with 9-digit fractional seconds.
 * Truncates fractional seconds to 3 digits and builds a local Date keeping camera numbers.
 */
export function parseCameraDate(isoStr: string): Date {
  if (!isoStr) return new Date();

  // Match: YYYY-MM-DD[T or space]HH:mm:ss(.fraction)?
  const match = isoStr.match(
    /^(\d{4})-(\d{2})-(\d{2})(?:[T\s](\d{2}):(\d{2})(?::(\d{2}))?(?:\.(\d+))?)?/,
  );

  if (match) {
    const [, year, month, day, hours = "0", minutes = "0", seconds = "0", fractions = "0"] = match;
    const ms = Number(fractions.slice(0, 3).padEnd(3, "0"));
    return new Date(
      Number(year),
      Number(month) - 1,
      Number(day),
      Number(hours),
      Number(minutes),
      Number(seconds),
      ms,
    );
  }

  // Fallback: strip excess fractional digits if present
  const cleaned = isoStr.replace(/(\.\d{3})\d+/, "$1");
  const parsed = new Date(cleaned);
  return Number.isNaN(parsed.getTime()) ? new Date() : parsed;
}

const idDateFormatter = new Intl.DateTimeFormat("id-ID", {
  day: "numeric",
  month: "short",
  year: "numeric",
});

const idTimeFormatter = new Intl.DateTimeFormat("id-ID", {
  hour: "2-digit",
  minute: "2-digit",
  hourCycle: "h23",
});

/** Format a single camera date into Indonesian readable format */
export function formatSingleDate(isoStr: string, includeTime = false): string {
  try {
    const d = parseCameraDate(isoStr);
    const datePart = idDateFormatter.format(d);
    if (!includeTime) return datePart;
    const timePart = idTimeFormatter.format(d).replace(":", ".");
    return `${datePart}, ${timePart}`;
  } catch {
    return isoStr;
  }
}

/** Format session start and end into a readable Indonesian date range */
export function formatSessionDateRange(startIso: string, endIso: string): string {
  try {
    const start = parseCameraDate(startIso);
    const end = parseCameraDate(endIso);

    const isSameDay =
      start.getFullYear() === end.getFullYear() &&
      start.getMonth() === end.getMonth() &&
      start.getDate() === end.getDate();

    const isSameTime = start.getTime() === end.getTime();

    if (isSameTime) {
      return formatSingleDate(startIso, true);
    }

    if (isSameDay) {
      const datePart = idDateFormatter.format(start);
      const startTime = idTimeFormatter.format(start).replace(":", ".");
      const endTime = idTimeFormatter.format(end).replace(":", ".");
      return `${datePart}, ${startTime} – ${endTime}`;
    }

    return `${idDateFormatter.format(start)} – ${idDateFormatter.format(end)}`;
  } catch {
    return `${startIso} – ${endIso}`;
  }
}

/**
 * Pure calculation of statistics across all sessions.
 * Highly optimized, runs in O(N).
 */
export function calculateSessionStats(sessions: Session[]): SessionStats {
  if (sessions.length === 0) {
    return {
      totalSessions: 0,
      activeCount: 0,
      archivedCount: 0,
      totalImages: undefined,
      overallMinEpoch: 0,
      overallMaxEpoch: 0,
      periodLabel: "—",
      durationLabel: "Belum ada sesi",
      latestSession: null,
      latestSessionDateLabel: "—",
    };
  }

  let activeCount = 0;
  let archivedCount = 0;
  let overallMinEpoch = Number.POSITIVE_INFINITY;
  let overallMaxEpoch = Number.NEGATIVE_INFINITY;
  let latestSession: Session | null = null;
  let latestEndEpoch = Number.NEGATIVE_INFINITY;

  for (const session of sessions) {
    if (session.status === "active") {
      activeCount += 1;
    } else {
      archivedCount += 1;
    }

    const startEpoch = parseCameraDate(session.dateStart).getTime();
    const endEpoch = parseCameraDate(session.dateEnd).getTime();

    if (startEpoch < overallMinEpoch) {
      overallMinEpoch = startEpoch;
    }
    if (endEpoch > overallMaxEpoch) {
      overallMaxEpoch = endEpoch;
    }

    if (endEpoch > latestEndEpoch) {
      latestEndEpoch = endEpoch;
      latestSession = session;
    }
  }

  // Ensure valid range even if dates were invalid
  if (!Number.isFinite(overallMinEpoch) || !Number.isFinite(overallMaxEpoch)) {
    const now = Date.now();
    overallMinEpoch = now;
    overallMaxEpoch = now;
  }

  const diffDays = Math.max(
    1,
    Math.round((overallMaxEpoch - overallMinEpoch) / (1000 * 60 * 60 * 24)) + 1,
  );

  const minDate = new Date(overallMinEpoch);
  const maxDate = new Date(overallMaxEpoch);

  const isSameDay =
    minDate.getFullYear() === maxDate.getFullYear() &&
    minDate.getMonth() === maxDate.getMonth() &&
    minDate.getDate() === maxDate.getDate();

  const periodLabel = isSameDay
    ? idDateFormatter.format(minDate)
    : `${idDateFormatter.format(minDate)} – ${idDateFormatter.format(maxDate)}`;

  const durationLabel = `${diffDays} hari`;

  const latestSessionDateLabel = latestSession
    ? formatSingleDate(latestSession.dateEnd, true)
    : "—";

  return {
    totalSessions: sessions.length,
    activeCount,
    archivedCount,
    totalImages: undefined, // Backend has no image list command yet
    overallMinEpoch,
    overallMaxEpoch,
    periodLabel,
    durationLabel,
    latestSession,
    latestSessionDateLabel,
  };
}

/**
 * Calculates percentage position and width for a session bar on the shared timeline axis.
 * Accurately positions single point-in-time sessions and extended duration sessions.
 */
export function calculateTimelinePosition(
  sessionStartEpoch: number,
  sessionEndEpoch: number,
  overallMinEpoch: number,
  overallMaxEpoch: number,
): TimelinePosition {
  const span = overallMaxEpoch - overallMinEpoch;

  // If the total project timeline span is negligible (< 30 seconds, e.g. only 1 session), show full span
  if (span < 30 * 1000) {
    return { leftPercent: 0, widthPercent: 100 };
  }

  const clampedStart = Math.max(overallMinEpoch, Math.min(overallMaxEpoch, sessionStartEpoch));
  const clampedEnd = Math.max(clampedStart, Math.min(overallMaxEpoch, sessionEndEpoch));

  const rawLeft = ((clampedStart - overallMinEpoch) / span) * 100;
  const rawWidth = ((clampedEnd - clampedStart) / span) * 100;

  // Enforce visible bar with minimum 6%
  const minWidthPercent = 6;
  const widthPercent = Math.max(
    minWidthPercent,
    Math.min(100 - rawLeft, rawWidth || minWidthPercent),
  );
  const leftPercent = Math.max(0, Math.min(100 - widthPercent, rawLeft));

  return { leftPercent, widthPercent };
}

export interface SessionProgressInfo {
  percent: number;
  label: string;
}

/**
 * Derives the workflow readiness progress and status message for a session.
 * - Freshly created (no distinct image timestamps): 20% ("Sesi dibuat · Menunggu citra")
 * - Connected images (distinct date range detected): 70% ("Citra terhubung · Siap dianalisis")
 * - Archived session: 100% ("Selesai · Sesi diarsipkan")
 */
export function getSessionProgressInfo(session: Session): SessionProgressInfo {
  if (session.status === "archived") {
    return {
      percent: 100,
      label: "Selesai | Sesi diarsipkan",
    };
  }

  const start = parseCameraDate(session.dateStart).getTime();
  const end = parseCameraDate(session.dateEnd).getTime();

  // Jika waktu mulai dan akhir berbeda, berarti ada rentang foto dengan timestamp EXIF riil
  const hasImageRange = !Number.isNaN(start) && !Number.isNaN(end) && start !== end;

  if (hasImageRange) {
    return {
      percent: 70,
      label: "Citra terhubung · Siap dianalisis",
    };
  }

  return {
    percent: 20,
    label: "Sesi dibuat | Menunggu citra",
  };
}
