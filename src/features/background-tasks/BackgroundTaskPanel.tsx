import { useEffect, useRef, useState } from "react";
import { Icon, type IconName } from "@blueprintjs/core";

import { useNow } from "../../hooks/useNow";
import { isLive, type TaskSnapshot, type TaskState } from "../../lib/backgroundTasks";
import { currentWork, groupOf, metaText, percentOf, type TaskGroup } from "./backgroundTaskFormat";

type Props = {
  tasks: TaskSnapshot[];
  canClear: boolean;
  onClose: () => void;
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

function TaskCard({
  task,
  now,
  expanded,
  onToggle,
  onCancel,
}: {
  task: TaskSnapshot;
  now: number;
  expanded: boolean;
  onToggle: () => void;
  onCancel: () => void;
}) {
  const live = isLive(task.state);
  const work = currentWork(task);
  const percent = percentOf(task);
  return (
    <div className={`bt-card${expanded ? " bt-card--open" : ""}`}>
      <div className="bt-card__head">
        <button type="button" className="bt-row" aria-expanded={expanded} onClick={onToggle}>
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
                <span className="bt-track__fill" style={{ width: `${percent}%` }} />
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
            onClick={onCancel}
          >
            <Icon icon="cross" size={14} />
          </button>
        )}
      </div>
      {expanded && (
        <div className="bt-detail">
          <span className="bt-detail__label">Pekerjaan saat ini</span>
          <div className="bt-detail__box">
            <div className="bt-detail__line">
              <span className="bt-detail__title">{work.title}</span>
              {task.state === "running" && <span className="bt-detail__pct">{percent}%</span>}
            </div>
            <span className="bt-detail__sub">{work.sub}</span>
            {task.state === "running" && (
              <span className="bt-track bt-track--thin" aria-hidden="true">
                <span className="bt-track__fill" style={{ width: `${percent}%` }} />
              </span>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

/**
 * Panel listing every background task in Berjalan / Antre / Selesai groups.
 * Escape and a click outside close it.
 */
export function BackgroundTaskPanel({
  tasks,
  canClear,
  onClose,
  onCancel,
  onClearFinished,
}: Props) {
  // `undefined` until the user opens or closes a card; until then the first
  // live task is shown expanded so its current work is visible at once.
  const [chosenId, setChosenId] = useState<string | null | undefined>(undefined);
  const expandedId =
    chosenId === undefined ? (tasks.find((task) => isLive(task.state))?.id ?? null) : chosenId;
  const panelRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    panelRef.current?.focus();
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  const liveCount = tasks.filter((task) => task.state === "running").length;
  const queuedCount = tasks.filter((task) => task.state === "queued").length;
  const activeCount = liveCount + queuedCount;
  // Ticks once a second only while something is running, for the elapsed time.
  const now = useNow(liveCount > 0);
  // No pill for an empty list: the body already says there is nothing.
  const pill = activeCount > 0 ? `${activeCount} aktif` : tasks.length > 0 ? "Selesai" : null;

  return (
    <>
      <button
        type="button"
        className="bt-backdrop"
        aria-label="Tutup panel tugas latar belakang"
        tabIndex={-1}
        onClick={onClose}
      />
      <div
        id="bt-panel"
        ref={panelRef}
        className="bt-panel"
        role="region"
        aria-label="Tugas latar belakang"
        tabIndex={-1}
      >
        <div className="bt-panel__head">
          <div className="bt-panel__heading">
            <h2 className="bt-panel__title">Tugas latar belakang</h2>
            {pill && (
              <span className={`bt-pill${activeCount > 0 ? " bt-pill--accent" : ""}`}>{pill}</span>
            )}
          </div>
          <div className="bt-panel__actions">
            {canClear && (
              <button type="button" className="bt-ghost bt-ghost--text" onClick={onClearFinished}>
                Bersihkan selesai
              </button>
            )}
            <button
              type="button"
              className="bt-ghost bt-ghost--icon"
              aria-label="Sembunyikan panel"
              onClick={onClose}
            >
              <Icon icon="cross" size={14} />
            </button>
          </div>
        </div>

        <div className="bt-panel__list">
          {tasks.length === 0 ? (
            <div className="bt-empty">
              <span className="bt-empty__icon">
                <Icon icon="tick" size={18} />
              </span>
              <span className="bt-empty__title">Tidak ada tugas latar belakang</span>
              <span className="bt-empty__sub">Pemrosesan citra akan muncul di sini.</span>
            </div>
          ) : (
            GROUPS.map((group) => {
              const inGroup = tasks.filter((task) => groupOf(task.state) === group);
              if (inGroup.length === 0) return null;
              return (
                <div key={group}>
                  <div className="bt-group">{group}</div>
                  {inGroup.map((task) => (
                    <TaskCard
                      key={task.id}
                      task={task}
                      now={now}
                      expanded={expandedId === task.id}
                      onToggle={() => setChosenId(expandedId === task.id ? null : task.id)}
                      onCancel={() => onCancel(task.id)}
                    />
                  ))}
                </div>
              );
            })
          )}
        </div>

        <div className="bt-panel__foot">Tugas tetap berjalan saat Anda berpindah halaman.</div>
      </div>
    </>
  );
}
