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
  /** Date part of the latest activity, e.g. "19 Okt 2026" (or "—"). */
  latestActivityDateLabel: string;
  /** Time part of the latest activity, e.g. "09.55" (or "" when none). */
  latestActivityTimeLabel: string;
}

export type SessionStage = 1 | 2 | 3;

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

/**
 * Parses a timezone-aware backend timestamp (`createdAt`/`updatedAt`, UTC with a `Z` suffix) into
 * the user's local time. Unlike `parseCameraDate`, which deliberately ignores any offset because
 * camera times are naive, this honours the offset.
 */
export function parseTimestamp(isoStr: string): Date {
  const parsed = new Date(isoStr.replace(/(\.\d{3})\d+/, "$1"));
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

/** Format a timezone-aware backend timestamp in the user's local time. */
export function formatTimestamp(isoStr: string, includeTime = false): string {
  const d = parseTimestamp(isoStr);
  const datePart = idDateFormatter.format(d);
  return includeTime ? `${datePart}, ${idTimeFormatter.format(d).replace(":", ".")}` : datePart;
}

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

/** Format the time part of a backend timestamp (local time) as "HH.mm" (Indonesian style). */
export function formatSingleTime(isoStr: string): string {
  try {
    return idTimeFormatter.format(parseTimestamp(isoStr)).replace(":", ".");
  } catch {
    return "";
  }
}

/**
 * Human-readable age of a timestamp relative to `nowMs`, e.g. "Hari ini" or "7 hari yang lalu".
 * Takes `nowMs` explicitly so the function stays pure.
 */
export function formatRelativeDays(isoStr: string, nowMs: number): string {
  const DAY_MS = 1000 * 60 * 60 * 24;
  const then = parseTimestamp(isoStr);
  if (Number.isNaN(then.getTime())) return "—";
  // Compare local calendar days, not elapsed 24h blocks: 23:00 yesterday is not "today".
  const startOfDay = (d: Date) => Date.UTC(d.getFullYear(), d.getMonth(), d.getDate());
  const days = Math.max(0, Math.round((startOfDay(new Date(nowMs)) - startOfDay(then)) / DAY_MS));
  return days === 0 ? "Hari ini" : `${days} hari yang lalu`;
}

/** Calendar-day span label ("N hari") and date label for an inclusive [min, max] epoch range. */
function describeRange(minEpoch: number, maxEpoch: number) {
  const minDate = new Date(minEpoch);
  const maxDate = new Date(maxEpoch);
  const dayStart = (d: Date) => Date.UTC(d.getFullYear(), d.getMonth(), d.getDate());
  const days = Math.round((dayStart(maxDate) - dayStart(minDate)) / (1000 * 60 * 60 * 24)) + 1;
  const periodLabel =
    days === 1
      ? idDateFormatter.format(minDate)
      : `${idDateFormatter.format(minDate)} – ${idDateFormatter.format(maxDate)}`;
  return { periodLabel, durationLabel: `${days} hari` };
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
      latestActivityDateLabel: "—",
      latestActivityTimeLabel: "",
    };
  }

  let activeCount = 0;
  let archivedCount = 0;
  let overallMinEpoch = Number.POSITIVE_INFINITY;
  let overallMaxEpoch = Number.NEGATIVE_INFINITY;
  let latestSession: Session | null = null;
  let latestEndEpoch = Number.NEGATIVE_INFINITY;
  let hasImageData = false;

  for (const session of sessions) {
    if (session.status === "active") {
      activeCount += 1;
    } else {
      archivedCount += 1;
    }

    const activityEpoch = parseTimestamp(session.updatedAt).getTime();

    // The shooting range only reflects sessions with connected images.
    if (hasImageRange(session)) {
      hasImageData = true;
      overallMinEpoch = Math.min(overallMinEpoch, parseCameraDate(session.dateStart).getTime());
      overallMaxEpoch = Math.max(overallMaxEpoch, parseCameraDate(session.dateEnd).getTime());
    }

    if (activityEpoch > latestEndEpoch) {
      latestEndEpoch = activityEpoch;
      latestSession = session;
    }
  }

  const { periodLabel, durationLabel } = hasImageData
    ? describeRange(overallMinEpoch, overallMaxEpoch)
    : { periodLabel: "Belum ada citra", durationLabel: "—" };

  const latestActivityDateLabel = latestSession ? formatTimestamp(latestSession.updatedAt) : "—";
  const latestActivityTimeLabel = latestSession ? formatSingleTime(latestSession.updatedAt) : "";

  return {
    totalSessions: sessions.length,
    activeCount,
    archivedCount,
    totalImages: undefined, // Backend has no image list command yet
    overallMinEpoch: hasImageData ? overallMinEpoch : 0,
    overallMaxEpoch: hasImageData ? overallMaxEpoch : 0,
    periodLabel,
    durationLabel,
    latestSession,
    latestActivityDateLabel,
    latestActivityTimeLabel,
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

/**
 * True when the session has connected images. The Session type has no image count, so this infers
 * it from the backend invariant: an empty session has `dateStart = dateEnd = createdAt` (naive UTC).
 * Any other range, including a single image or images sharing one timestamp, means images exist.
 */
export function hasImageRange(session: Session): boolean {
  const start = parseCameraDate(session.dateStart);
  const end = parseCameraDate(session.dateEnd);
  const created = parseTimestamp(session.createdAt).getTime();
  const startAsUtc = Date.UTC(
    start.getFullYear(),
    start.getMonth(),
    start.getDate(),
    start.getHours(),
    start.getMinutes(),
    start.getSeconds(),
  );
  const isPlaceholderRange =
    start.getTime() === end.getTime() && Math.abs(startAsUtc - created) < 2000;
  return !isPlaceholderRange;
}

/**
 * Lifecycle step shown in the session card stepper:
 * 1 = Sesi dibuat, 2 = Citra ditambahkan, 3 = Selesai & diarsipkan (archived).
 */
export function getSessionStage(session: Session): SessionStage {
  if (session.status === "archived") return 3;
  // An active session is past "Sesi dibuat" by definition: it waits on images (2) or on archiving (3).
  return hasImageRange(session) ? 3 : 2;
}
