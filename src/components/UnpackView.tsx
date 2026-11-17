import type { Job } from "../lib/model";
import { formatBytes, isFinished, phaseText } from "../lib/model";
import { Close, Reveal, Retry, StatusGlyph, Stop, Tray } from "./Icons";
import { Kbd } from "./Kbd";

interface Props {
  jobs: Job[];
  selected: number | null;
  destinationHint: string;
  onSelect: (id: number) => void;
  onOpen: () => void;
  onReveal: (job: Job) => void;
  onCancel: (job: Job) => void;
  onRetry: (job: Job) => void;
  onRemove: (job: Job) => void;
  onClearFinished: () => void;
}

/**
 * The first thing anyone sees says what the app does: drop archives here.
 * Once there are archives, each is one row, newest first, and every
 * finished one says where its files went.
 */
export function UnpackView(p: Props) {
  if (p.jobs.length === 0) {
    return (
      <div className="list-wrap">
        <button type="button" className="dropzone" onClick={p.onOpen}>
          <Tray className="dropzone-icon" />
          <span className="dropzone-title">Drop archives here to unpack them</span>
          <span className="dropzone-detail">ZIP, RAR, 7z, TAR, GZ, BZ2, XZ and ZST, including split and password-protected archives.</span>
          <span className="dropzone-detail">{p.destinationHint}</span>
          <span className="btn primary dropzone-btn">
            Choose archives… <Kbd keys={["⌘", "O"]} />
          </span>
        </button>
      </div>
    );
  }

  const finished = p.jobs.filter(isFinished).length;
  return (
    <div className="list-wrap">
      <section className="group">
        <div className="group-head">
          <span>Archives</span>
          <span className="group-count">{p.jobs.length}</span>
          <span className="row-fill" />
          {finished > 0 && (
            <button type="button" className="link small" onClick={p.onClearFinished}>
              Clear finished
            </button>
          )}
        </div>
        <ul className="list">
          {p.jobs.map((job) => (
            <JobRow key={job.id} {...p} job={job} isSelected={p.selected === job.id} />
          ))}
        </ul>
      </section>
    </div>
  );
}

function JobRow({ job, isSelected, onSelect, onReveal, onCancel, onRetry, onRemove }: Props & { job: Job; isSelected: boolean }) {
  const s = job.state;
  const tone = s.phase === "done" ? " p-done" : s.phase === "failed" ? " p-failed" : s.phase === "needsPassword" ? " p-attention" : "";
  return (
    <li
      className={`row job${isSelected ? " selected" : ""}`}
      onClick={() => onSelect(job.id)}
      onDoubleClick={() => s.phase === "done" && onReveal(job)}
    >
      <StatusGlyph state={s} />
      <span className="row-name" title={job.path}>
        {job.name}
      </span>
      {job.listing && <span className="fmt-pill">{job.listing.formatLabel}</span>}
      {job.listing && <span className="row-size mono">{formatBytes(job.listing.archiveSize)}</span>}
      <span className="row-fill" />
      <span className={`phase${tone}`} title={phaseText(job)}>
        {phaseText(job)}
      </span>
      <span className="row-actions" onClick={(e) => e.stopPropagation()}>
        {s.phase === "done" && (
          <button type="button" className="btn small" onClick={() => onReveal(job)} title="Show in Finder">
            <Reveal className="i" /> Show in Finder
          </button>
        )}
        {(s.phase === "extracting" || s.phase === "waiting" || s.phase === "reading") && (
          <button type="button" className="icon-btn" onClick={() => onCancel(job)} title="Stop">
            <Stop className="i" />
          </button>
        )}
        {(s.phase === "failed" || s.phase === "cancelled") && (
          <button type="button" className="icon-btn" onClick={() => onRetry(job)} title="Try again">
            <Retry className="i" />
          </button>
        )}
        {isFinished(job) && (
          <button type="button" className="icon-btn" onClick={() => onRemove(job)} title="Remove from list">
            <Close className="i" />
          </button>
        )}
      </span>
      {s.phase === "extracting" && s.total > 0 && (
        <span className="row-progress" style={{ width: `${Math.min(100, (s.done / s.total) * 100)}%` }} />
      )}
    </li>
  );
}
