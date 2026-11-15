import { Icon } from "@blueprintjs/core";

import { barLabel, percentOf, type BarSummary } from "./backgroundTaskFormat";

type Props = {
  summary: BarSummary;
  open: boolean;
  onToggle: () => void;
  /** Opens the panel; used by the failed-tasks shortcut. */
  onOpen: () => void;
};

/**
 * Slim status bar along the bottom of the window. Its button shows what the
 * background tasks are doing and opens {@link BackgroundTaskPanel}.
 */
export function BackgroundTaskStatusBar({ summary, open, onToggle, onOpen }: Props) {
  const { primary, others, failed } = summary;
  return (
    <footer className="bt-bar" role="contentinfo" aria-label="Bilah status">
      {failed > 0 && (
        <button type="button" className="bt-bar__item bt-bar__item--failed" onClick={onOpen}>
          <Icon icon="error" size={13} />
          {failed} gagal
        </button>
      )}
      <button
        type="button"
        className={`bt-bar__item${open ? " bt-bar__item--open" : ""}`}
        aria-controls="bt-panel"
        aria-expanded={open}
        aria-label={barLabel(summary, open)}
        onClick={onToggle}
      >
        {primary ? (
          <span className="bt-spin bt-spin--small" aria-hidden="true" />
        ) : summary.empty ? (
          <Icon icon="dot" size={13} className="bt-bar__none" />
        ) : (
          <Icon icon="tick" size={13} className="bt-bar__idle" />
        )}
        <span aria-live="polite" className="bt-bar__text">
          {summary.text}
        </span>
        {primary && (
          <span className="bt-bar__track" aria-hidden="true">
            <span className="bt-bar__fill" style={{ width: `${percentOf(primary)}%` }} />
          </span>
        )}
        {others > 0 && <span className="bt-pill bt-pill--accent">+{others} tugas</span>}
        <Icon
          icon="chevron-up"
          size={14}
          className={`bt-bar__chev${open ? " bt-bar__chev--open" : ""}`}
        />
      </button>
    </footer>
  );
}
