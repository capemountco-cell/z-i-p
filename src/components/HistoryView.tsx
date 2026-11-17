import type { HistoryItem } from "../lib/native";
import { fileName, formatBytes, plural, timeAgo } from "../lib/model";
import { CheckCircle, History, Warning } from "./Icons";

interface Props {
  items: HistoryItem[];
  onReveal: (path: string) => void;
  onClear: () => void;
}

/** Everything unpacked, newest first: "where did my files go?" answered. */
export function HistoryView({ items, onReveal, onClear }: Props) {
  if (items.length === 0) {
    return (
      <div className="empty">
        <History className="empty-icon" />
        <p>Nothing unpacked yet</p>
        <p className="faint">Every archive you unpack is listed here, with where its files went.</p>
      </div>
    );
  }
  return (
    <div className="list-wrap">
      <section className="group">
        <div className="group-head">
          <span>Unpacked</span>
          <span className="group-count">{items.length}</span>
          <span className="row-fill" />
          <button type="button" className="link small" onClick={onClear}>
            Clear history
          </button>
        </div>
        <ul className="list">
          {items.map((item) => (
            <li
              key={`${item.at}-${item.archive}`}
              className={`row${item.error ? "" : " clickable"}`}
              title={item.error ?? `Show ${item.output} in Finder`}
              onClick={() => !item.error && onReveal(item.output)}
            >
              {item.error ? <Warning className="i s-failed" /> : <CheckCircle className="i s-done" />}
              <span className="row-name">{fileName(item.archive)}</span>
              {!item.error && (
                <>
                  <span className="arrow">→</span>
                  <span className="row-source">{fileName(item.output)}</span>
                </>
              )}
              <span className="row-fill" />
              <span className={`phase${item.error ? " p-failed" : ""}`}>
                {item.error ?? `${plural(item.files, "file")} · ${formatBytes(item.bytes)}`}
              </span>
              <span className="row-time faint">{timeAgo(item.at)}</span>
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}
