import { isLive, type TaskSnapshot, type TaskState } from "../../lib/backgroundTasks";

/** Which panel group a task state belongs to. */
export type TaskGroup = "Berjalan" | "Antre" | "Selesai";

export function groupOf(state: TaskState): TaskGroup {
  if (state === "running") return "Berjalan";
  if (state === "queued") return "Antre";
  return "Selesai";
}

/** `HH.mm` in local time, the way the design writes clock times. */
export function formatClock(iso: string | null): string {
  if (!iso) return "";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(date.getHours())}.${pad(date.getMinutes())}`;
}

/** Percent complete, 0–100, rounded. An empty task counts as 0. */
export function percentOf(task: Pick<TaskSnapshot, "done" | "total">): number {
  if (task.total <= 0) return 0;
  return Math.min(100, Math.round((task.done / task.total) * 100));
}

/**
 * How long a task has been running, in milliseconds. A finished task is
 * measured start to finish; a live one up to `now`. Never negative, and 0 when
 * a timestamp cannot be read.
 */
export function elapsedMs(
  task: Pick<TaskSnapshot, "startedAt" | "finishedAt">,
  now: number,
): number {
  const start = Date.parse(task.startedAt);
  const end = task.finishedAt ? Date.parse(task.finishedAt) : now;
  if (Number.isNaN(start) || Number.isNaN(end)) return 0;
  return Math.max(0, end - start);
}

/**
 * A duration in Indonesian, e.g. `1 menit 12 detik`, `45 detik`, `2 jam`.
 * Seconds are rounded down; under one second reads `kurang dari 1 detik`.
 */
export function formatDuration(ms: number): string {
  const total = Math.floor(Math.max(0, ms) / 1000);
  if (total < 1) return "kurang dari 1 detik";
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const seconds = total % 60;
  const parts: string[] = [];
  if (hours > 0) parts.push(`${hours} jam`);
  if (minutes > 0) parts.push(`${minutes} menit`);
  // Seconds are noise once an hour has passed.
  if (seconds > 0 && hours === 0) parts.push(`${seconds} detik`);
  return parts.join(" ");
}

/** ` · N DNG dilewati`, or `""` when nothing was skipped. */
function skippedSuffix(task: Pick<TaskSnapshot, "skipped">): string {
  return task.skipped > 0 ? ` · ${task.skipped} DNG dilewati` : "";
}

/** The one-line status under a task's title. `now` is the current time in ms. */
export function metaText(task: TaskSnapshot, now: number): string {
  const scored = Math.max(0, task.done - task.unscorable);
  const took = formatDuration(elapsedMs(task, now));
  const skipped = skippedSuffix(task);
  // A skip-only pass (standalone DNGs only, nothing queued to score) has no
  // scoring work at all, so it gets its own wording rather than "0 dari 0".
  if (task.state === "done" && task.total === 0) {
    return `Tidak ada citra untuk dinilai${skipped}`;
  }
  switch (task.state) {
    case "queued":
      return "Menunggu giliran";
    case "running":
      return `${task.done} dari ${task.total} citra dinilai · ${took}${skipped}`;
    case "done":
      return `${task.total} citra dinilai selama ${took} · selesai ${formatClock(task.finishedAt)}${skipped}`;
    case "warn":
      return `${scored} dari ${task.total} citra dinilai selama ${took} · ${task.unscorable} tidak dapat dinilai${skipped}`;
    case "cancelled":
      return `Dibatalkan setelah ${took} · ${task.done} dari ${task.total} citra dinilai${skipped}`;
    case "failed":
      return `Gagal setelah ${took} · ${task.error ?? "kesalahan tidak diketahui"}`;
  }
}

/** Why standalone DNGs were skipped, for the expanded detail's sub line. */
const SKIPPED_REASON =
  "citra DNG tanpa pasangan JPEG tidak dinilai; ketajamannya tidak dapat diukur dari pratinjau.";

/** The "Pekerjaan saat ini" box shown when a task card is expanded. */
export function currentWork(task: TaskSnapshot): { title: string; sub: string } {
  const skippedNote = task.skipped > 0 ? `${task.skipped} ${SKIPPED_REASON}` : "";
  const withSkipped = (sub: string) => (skippedNote ? `${sub} ${skippedNote}` : sub);
  switch (task.state) {
    case "queued":
      return { title: "Belum dimulai", sub: "Mulai otomatis setelah tugas sebelumnya selesai." };
    case "running":
      return {
        title: task.current ? `Terakhir dinilai: ${task.current}` : "Memulai penilaian…",
        sub: `${task.done} dari ${task.total} citra`,
      };
    case "done":
      if (task.total === 0) {
        // Skip-only pass: nothing was ever queued to score.
        return { title: "Tidak ada pekerjaan", sub: skippedNote };
      }
      return { title: "Tidak ada pekerjaan", sub: withSkipped("Semua citra sudah dinilai.") };
    case "warn":
      return {
        title: "Tidak ada pekerjaan",
        sub: withSkipped(
          "Beberapa citra terlalu kecil atau tidak dapat dibaca, sehingga tidak mendapat skor.",
        ),
      };
    case "cancelled":
      return {
        title: "Tidak ada pekerjaan",
        sub: withSkipped("Penilaian dihentikan. Sisa citra dilanjutkan saat aplikasi dibuka lagi."),
      };
    case "failed":
      return { title: "Tidak ada pekerjaan", sub: task.error ?? "Penilaian gagal." };
  }
}

/** What the status bar button says, and how much is going on behind it. */
export type BarSummary = {
  text: string;
  /** No task exists at all (as opposed to every task being finished). */
  empty: boolean;
  /** The task whose progress the bar shows, if any are live. */
  primary: TaskSnapshot | null;
  /** Live tasks beyond the primary one. */
  others: number;
  failed: number;
};

export function summarize(tasks: TaskSnapshot[]): BarSummary {
  const live = tasks.filter((task) => isLive(task.state));
  const primary = live.find((task) => task.state === "running") ?? live[0] ?? null;
  return {
    text: primary
      ? `Menilai ketajaman · ${primary.done}/${primary.total}`
      : tasks.length === 0
        ? "Tidak ada tugas latar belakang"
        : "Semua tugas selesai",
    empty: tasks.length === 0,
    primary,
    others: live.length - (primary ? 1 : 0),
    failed: tasks.filter((task) => task.state === "failed").length,
  };
}

/** The accessible name of the status bar button. */
export function barLabel(summary: BarSummary, open: boolean): string {
  const more = summary.others > 0 ? `, ${summary.others} tugas lain` : "";
  return `Tugas latar belakang: ${summary.text}${more}. Klik untuk ${
    open ? "menyembunyikan" : "melihat"
  } detail.`;
}
