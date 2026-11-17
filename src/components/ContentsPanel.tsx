import { useMemo, useState } from "react";

import type { Job } from "../lib/model";
import { depth, fileCount, filterEntries, formatBytes, normaliseSelection, phaseText, selectionSize, tildify } from "../lib/model";
import { Close, File, Folder, Lock, Reveal, Search } from "./Icons";

interface Props {
  job: Job;
  onClose: () => void;
  onExtract: (job: Job, only: string[] | null) => void;
  onReveal: (job: Job) => void;
  onChooseDestination: (job: Job) => void;
}

/**
 * What is inside an archive, with a box beside every file and folder, so a
 * person can take one file out of a 10 GB archive. Shown beside the list,
 * like Camu's detail panel.
 */
export function ContentsPanel({ job, onClose, onExtract, onReveal, onChooseDestination }: Props) {
  const [query, setQuery] = useState("");
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const listing = job.listing;
  const entries = useMemo(() => (listing ? filterEntries(listing.entries, query) : []), [listing, query]);
  const selection = normaliseSelection(picked);
  const busy = ["reading", "waiting", "extracting"].includes(job.state.phase);
  const shown = entries.slice(0, 2000);

  const toggle = (path: string) => {
    const next = new Set(picked);
    if (next.has(path)) next.delete(path);
    else next.add(path);
    setPicked(next);
  };

  return (
    <aside className="panel" aria-label="Archive contents">
      <div className="panel-head">
        <h2 title={job.path}>{job.name}</h2>
        <button type="button" className="icon-btn" onClick={onClose} title="Close (Esc)">
          <Close className="i" />
        </button>
      </div>
      <div className="panel-body">
        {listing ? (
          <dl className="props">
            <dt>Format</dt>
            <dd>
              {listing.formatLabel}
              {listing.pieces > 1 ? ` · ${listing.pieces} pieces` : ""}
            </dd>
            <dt>Size</dt>
            <dd>
              {formatBytes(listing.archiveSize)}
              {listing.totalSize != null ? ` → ${formatBytes(listing.totalSize)} unpacked` : ""}
            </dd>
            <dt>Contains</dt>
            <dd>{fileCount(listing.entries).toLocaleString()} files</dd>
            {listing.encrypted && (
              <>
                <dt>Protection</dt>
                <dd className="with-icon">
                  <Lock className="i" /> Encrypted with a password
                </dd>
              </>
            )}
            {listing.unsafeNames > 0 && (
              <>
                <dt>Left out</dt>
                <dd>{listing.unsafeNames} with names that point outside the folder</dd>
              </>
            )}
            <dt>Unpacks to</dt>
            <dd>
              <button type="button" className="link" onClick={() => onChooseDestination(job)} title="Choose another folder">
                {job.destination ? tildify(job.destination) : "Default place"}
              </button>
            </dd>
            <dt>Status</dt>
            <dd>{phaseText(job)}</dd>
          </dl>
        ) : (
          <p className="muted">{phaseText(job)}</p>
        )}

        {listing && listing.entries.length > 0 && (
          <section className="section contents">
            <label className="filter-input">
              <Search className="i" />
              <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Filter files" spellCheck={false} />
            </label>
            <ul className="tree" role="listbox" aria-multiselectable>
              {shown.map((e) => (
                <li key={e.path} className="tree-row" style={{ paddingLeft: 8 + (query ? 0 : depth(e.path) * 14) }}>
                  <label>
                    <input type="checkbox" checked={picked.has(e.path)} onChange={() => toggle(e.path)} />
                    {e.isDir ? <Folder className="i faint" /> : <File className="i faint" />}
                    <span className="tree-name" title={e.path}>
                      {query ? e.path : e.path.split("/").pop()}
                    </span>
                    {!e.isDir && <span className="tree-size mono">{formatBytes(e.size)}</span>}
                  </label>
                </li>
              ))}
              {entries.length > shown.length && <li className="tree-more faint">and {entries.length - shown.length} more — filter to find them</li>}
            </ul>
          </section>
        )}

        <div className="panel-actions">
          {job.state.phase === "done" && (
            <button type="button" className="btn wide" onClick={() => onReveal(job)}>
              <Reveal className="i" /> Show in Finder
            </button>
          )}
          {listing && selection.length > 0 && (
            <button type="button" className="btn primary wide" disabled={busy} onClick={() => onExtract(job, selection)}>
              Unpack {selection.length} selected · {formatBytes(selectionSize(listing.entries, selection))}
            </button>
          )}
          {listing && (
            <button type="button" className={`btn wide${selection.length ? "" : " primary"}`} disabled={busy} onClick={() => onExtract(job, null)}>
              Unpack everything
            </button>
          )}
        </div>
      </div>
    </aside>
  );
}
