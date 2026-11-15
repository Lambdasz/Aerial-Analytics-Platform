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

/** The one-line status under a task's title. */
export function metaText(task: TaskSnapshot): string {
  const scored = Math.max(0, task.done - task.unscorable);
  switch (task.state) {
    case "queued":
      return "Menunggu giliran";
    case "running":
      return `${task.done} dari ${task.total} citra dinilai`;
    case "done":
      return `${task.total} citra dinilai · selesai ${formatClock(task.finishedAt)}`;
    case "warn":
      return `${scored} dari ${task.total} citra dinilai · ${task.unscorable} tidak dapat dinilai`;
    case "cancelled":
      return `Dibatalkan · ${task.done} dari ${task.total} citra dinilai`;
    case "failed":
      return `Gagal · ${task.error ?? "kesalahan tidak diketahui"}`;
  }
}

/** The "Pekerjaan saat ini" box shown when a task card is expanded. */
export function currentWork(task: TaskSnapshot): { title: string; sub: string } {
  switch (task.state) {
    case "queued":
      return { title: "Belum dimulai", sub: "Mulai otomatis setelah tugas sebelumnya selesai." };
    case "running":
      return {
        title: task.current ? `Terakhir dinilai: ${task.current}` : "Memulai penilaian…",
        sub: `${task.done} dari ${task.total} citra`,
      };
    case "done":
      return { title: "Tidak ada pekerjaan", sub: "Semua citra sudah dinilai." };
    case "warn":
      return {
        title: "Tidak ada pekerjaan",
        sub: "Beberapa citra terlalu kecil atau tidak dapat dibaca, sehingga tidak mendapat skor.",
      };
    case "cancelled":
      return {
        title: "Tidak ada pekerjaan",
        sub: "Penilaian dihentikan. Sisa citra dilanjutkan saat aplikasi dibuka lagi.",
      };
    case "failed":
      return { title: "Tidak ada pekerjaan", sub: task.error ?? "Penilaian gagal." };
  }
}

/** What the status bar button says, and how much is going on behind it. */
export type BarSummary = {
  text: string;
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
    text: primary ? `Menilai ketajaman · ${primary.done}/${primary.total}` : "Semua tugas selesai",
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
