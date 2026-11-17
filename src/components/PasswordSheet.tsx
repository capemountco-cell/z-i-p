import { useEffect, useRef, useState } from "react";

import type { Job } from "../lib/model";
import { Lock } from "./Icons";

interface Props {
  job: Job;
  waiting: number;
  onSubmit: (password: string) => void;
  onCancel: () => void;
}

/**
 * Says plainly that the archive is protected, and that the password will be
 * tried on the other locked archives in this batch, so it is typed once.
 * Passwords are kept in memory only, never on disk.
 */
export function PasswordSheet({ job, waiting, onSubmit, onCancel }: Props) {
  const [value, setValue] = useState("");
  const [show, setShow] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  const wrong = job.state.phase === "needsPassword" && job.state.wrong;

  useEffect(() => {
    setValue("");
    input.current?.focus();
  }, [job.id, wrong]);

  return (
    <div className="scrim" onMouseDown={onCancel}>
      <form
        className="sheet"
        onMouseDown={(e) => e.stopPropagation()}
        onSubmit={(e) => {
          e.preventDefault();
          if (value) onSubmit(value);
        }}
      >
        <div className="sheet-head">
          <Lock className="i sheet-icon" />
          <div>
            <h2>“{job.name}” is protected</h2>
            <p className="muted">Enter its password to unpack it.</p>
          </div>
        </div>
        <input
          ref={input}
          className={`text-input${wrong ? " invalid" : ""}`}
          type={show ? "text" : "password"}
          value={value}
          onChange={(e) => setValue(e.target.value)}
          placeholder="Password"
          aria-label="Password"
          autoComplete="off"
          spellCheck={false}
        />
        {wrong && <p className="danger small-text">That password doesn't open it. Check the spelling and caps lock.</p>}
        <label className="check-line">
          <input type="checkbox" checked={show} onChange={(e) => setShow(e.target.checked)} /> Show password
        </label>
        {waiting > 0 && (
          <p className="faint small-text">
            It will also be tried on {waiting} other locked {waiting === 1 ? "archive" : "archives"} in this batch.
          </p>
        )}
        <div className="sheet-actions">
          <button type="button" className="btn" onClick={onCancel}>
            Skip
          </button>
          <button type="submit" className="btn primary" disabled={!value}>
            Unpack
          </button>
        </div>
      </form>
    </div>
  );
}
