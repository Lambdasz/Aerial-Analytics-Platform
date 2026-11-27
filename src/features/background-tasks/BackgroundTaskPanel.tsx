import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Icon, type IconName } from "@blueprintjs/core";

import { useNow } from "../../hooks/useNow";
import { isLive, type TaskSnapshot, type TaskState } from "../../lib/backgroundTasks";
import { currentWork, groupOf, metaText, percentOf, type TaskGroup } from "./backgroundTaskFormat";

type Props = {
  tasks: TaskSnapshot[];
  canClear: boolean;
  onCancel: (taskId: string) => void;
  onClearFinished: () => void;
};

const GROUPS: TaskGroup[] = ["Berjalan", "Antre", "Selesai"];

const STATE_ICON: Record<Exclude<TaskState, "running">, IconName> = {
  queued: "time",
  done: "tick",
  warn: "warning-sign",
  cancelled: "disable",
  failed: "error",
};

function StateIcon({ state }: { state: TaskState }) {
  return (
    <span className={`bt-icon bt-icon--${state}`}>
      {state === "running" ? (
        <span className="bt-spin" aria-hidden="true" />
      ) : (
        <Icon icon={STATE_ICON[state]} size={14} />
      )}
    </span>
  );
}

/**
 * Keeps something mounted for `ms` after `show` turns false, so a closing
 * animation can finish before the content is removed.
 */
function useDelayedUnmount(show: boolean, ms: number): boolean {
  const [mounted, setMounted] = useState(show);
  if (show && !mounted) setMounted(true);
  useEffect(() => {
    if (show) return;
    const timer = window.setTimeout(() => setMounted(false), ms);
    return () => window.clearTimeout(timer);
  }, [show, ms]);
  return show || mounted;
}

type CardProps = {
  task: TaskSnapshot;
  /** The clock, only for running tasks (they show elapsed time); 0 otherwise. */
  now: number;
  expanded: boolean;
  onToggle: (taskId: string) => void;
  onCancel: (taskId: string) => void;
};

/**
 * One task. The header and the expanded detail are a single bordered card.
 * Memoised: with stable handlers and `now` fixed at 0 for non-running tasks,
 * only running cards re-render on the once-a-second clock tick.
 */
const TaskCard = memo(function TaskCard({ task, now, expanded, onToggle, onCancel }: CardProps) {
  const live = isLive(task.state);
  const percent = percentOf(task);
  const fill = { transform: `scaleX(${percent / 100})` };
  // Detail content exists only while open (or closing), to keep the DOM small.
  const showDetail = useDelayedUnmount(expanded, 300);
  const work = showDetail ? currentWork(task) : null;
  return (
    <div className={`bt-card bt-card--${task.state}${expanded ? " bt-card--open" : ""}`}>
      <div className="bt-card__head">
        <button
          type="button"
          className="bt-row"
          aria-expanded={expanded}
          aria-controls={`bt-detail-${task.id}`}
          // Hover anywhere on the row for the full, untruncated name.
          title={`${task.title}\n${task.sessionName}`}
          // Mouse clicks shouldn't move focus onto the row (no ring after a click);
          // keyboard focus and Enter/Space still work as usual.
          onMouseDown={(event) => event.preventDefault()}
          onClick={() => onToggle(task.id)}
        >
          <StateIcon state={task.state} />
          <span className="bt-row__body">
            <span className="bt-row__title">{task.title}</span>
            <span className="bt-row__scope">{task.sessionName}</span>
            {task.state === "running" && (
              <span
                className="bt-track"
                role="progressbar"
                aria-label={task.title}
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={percent}
              >
                <span className="bt-track__fill" style={fill} />
              </span>
            )}
            <span className={`bt-row__meta${task.state === "warn" ? " bt-row__meta--warn" : ""}`}>
              {metaText(task, now)}
            </span>
          </span>
          <Icon
            icon="chevron-down"
            size={14}
            className={`bt-chev${expanded ? " bt-chev--open" : ""}`}
          />
        </button>
        {live && (
          <button
            type="button"
            className="bt-ghost bt-ghost--icon"
            aria-label={`Batalkan ${task.title.toLowerCase()}`}
            title={`Batalkan ${task.title.toLowerCase()}`}
            onClick={() => onCancel(task.id)}
          >
            <Icon icon="cross" size={14} />
          </button>
        )}
      </div>
      {/* Always present so its height can animate; `inert` keeps it out of reach when closed. */}
      <div className="bt-detail-wrap" id={`bt-detail-${task.id}`} inert={!expanded}>
        <div className="bt-detail-clip">
          {work && (
            <div className="bt-detail">
              <span className="bt-detail__label">Pekerjaan saat ini</span>
              <div className="bt-detail__line">
                <span className="bt-detail__title">{work.title}</span>
                {task.state === "running" && <span className="bt-detail__pct">{percent}%</span>}
              </div>
              <span className="bt-detail__sub">{work.sub}</span>
              {task.state === "running" && (
                <span className="bt-track bt-track--thin" aria-hidden="true">
                  <span className="bt-track__fill" style={fill} />
                </span>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
});

/** One pass over the tasks: grouped, counted, and the first live one found. */
function organize(tasks: TaskSnapshot[]) {
  const groups: Record<TaskGroup, TaskSnapshot[]> = { Berjalan: [], Antre: [], Selesai: [] };
  let running = 0;
  let queued = 0;
  let failed = 0;
  let finished = 0;
  let firstLiveId: string | null = null;
  for (const task of tasks) {
    groups[groupOf(task.state)].push(task);
    if (firstLiveId === null && isLive(task.state)) firstLiveId = task.id;
    if (task.state === "running") running += 1;
    else if (task.state === "queued") queued += 1;
    else {
      finished += 1;
      if (task.state === "failed") failed += 1;
    }
  }
  return { groups, running, queued, failed, finished, firstLiveId };
}

/** Screen-reader summary. Changes only when a task changes state, not on progress. */
function spokenSummary(counts: {
  running: number;
  queued: number;
  failed: number;
  finished: number;
}) {
  const { running, queued, failed, finished } = counts;
  const parts: string[] = [];
  if (running > 0) parts.push(`${running} berjalan`);
  if (queued > 0) parts.push(`${queued} antre`);
  if (failed > 0) parts.push(`${failed} gagal`);
  if (finished - failed > 0) parts.push(`${finished - failed} selesai`);
  return parts.length > 0 ? parts.join(", ") : "Tidak ada tugas latar belakang";
}

/**
 * Sidebar section listing every background task in Berjalan / Antre / Selesai
 * groups. Sits at the bottom of the sidebar; grows with its content but never
 * past the space under the menu, then the list scrolls.
 */
export function BackgroundTaskPanel({ tasks, canClear, onCancel, onClearFinished }: Props) {
  // `undefined` until the user opens or closes a card; until then the first
  // live task is shown expanded so its current work is visible at once.
  const [chosenId, setChosenId] = useState<string | null | undefined>(undefined);

  const organized = useMemo(() => organize(tasks), [tasks]);
  const { groups, running, queued, failed, firstLiveId } = organized;
  const expandedId = chosenId === undefined ? firstLiveId : chosenId;

  const toggle = useCallback(
    (taskId: string) => {
      setChosenId((previous) => {
        const current = previous === undefined ? firstLiveId : previous;
        return current === taskId ? null : taskId;
      });
    },
    [firstLiveId],
  );

  // Latest-callback ref, so cards get a handler that never changes identity.
  const cancelRef = useRef(onCancel);
  useEffect(() => {
    cancelRef.current = onCancel;
  });
  const cancel = useCallback((taskId: string) => cancelRef.current(taskId), []);

  const active = running + queued;
  // Ticks once a second only while something is running, for the elapsed time.
  const now = useNow(running > 0);
  // No pill for an empty list: the body already says there is nothing.
  const pill = active > 0 ? `${active} aktif` : tasks.length > 0 ? "Selesai" : null;
  const summary = spokenSummary(organized);

  return (
    <section className="bt-section" aria-label="Background Tasks">
      <p className="bt-sr" role="status">
        {summary}
      </p>

      <div className="bt-section__head">
        <div className="bt-section__bar">
          <h2 className="bt-section__title">
            Background Tasks
            <span className="bt-info">
              <button
                type="button"
                className="bt-ghost bt-ghost--icon"
                aria-label="Tentang tugas latar belakang"
                aria-describedby="bt-info-tip"
              >
                <span className="bt-info__badge" aria-hidden="true">
                  i
                </span>
              </button>
              <span id="bt-info-tip" role="tooltip" className="bt-info__tip">
                Tugas tetap berjalan saat Anda berpindah halaman.
              </span>
            </span>
          </h2>
          <div className="bt-section__tools">
            {canClear && (
              <button
                type="button"
                className="bt-ghost bt-ghost--icon"
                aria-label="Bersihkan selesai"
                title="Bersihkan selesai"
                onClick={onClearFinished}
              >
                <Icon icon="trash" size={14} />
              </button>
            )}
          </div>
        </div>
        {(pill || failed > 0) && (
          <div className="bt-section__pills">
            {pill && (
              <span className={`bt-pill${active > 0 ? " bt-pill--accent" : ""}`}>{pill}</span>
            )}
            {failed > 0 && <span className="bt-pill bt-pill--danger">{failed} gagal</span>}
          </div>
        )}
      </div>

      <div className="bt-section__list">
        {tasks.length === 0 ? (
          <div className="bt-empty">
            <span className="bt-zzz" aria-hidden="true">
              <span>z</span>
              <span>z</span>
              <span>Z</span>
            </span>
            <span className="bt-empty__title">Tidak ada tugas latar belakang</span>
            <span className="bt-empty__sub">Pemrosesan citra akan muncul di sini.</span>
          </div>
        ) : (
          GROUPS.map((group) =>
            groups[group].length === 0 ? null : (
              <div key={group}>
                <div className="bt-group">{group}</div>
                {groups[group].map((task) => (
                  <TaskCard
                    key={task.id}
                    task={task}
                    now={task.state === "running" ? now : 0}
                    expanded={expandedId === task.id}
                    onToggle={toggle}
                    onCancel={cancel}
                  />
                ))}
              </div>
            ),
          )
        )}
      </div>
    </section>
  );
}
