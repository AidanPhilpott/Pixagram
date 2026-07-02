import { useState, type ReactNode } from "react";

interface CollapsibleSectionProps {
  /** Stable key for the persisted open/closed state. */
  id: string;
  title: string;
  /** Initial state when nothing is persisted yet. */
  defaultOpen?: boolean;
  children: ReactNode;
}

const storageKey = (id: string) => `pixagram.section.${id}`;

/**
 * Inspector disclosure group: a chevron header that shows/hides its children.
 * Open/closed state persists per section in localStorage so the layout the
 * user left behind survives restarts.
 */
export function CollapsibleSection({
  id,
  title,
  defaultOpen = false,
  children,
}: CollapsibleSectionProps) {
  const [open, setOpen] = useState(() => {
    try {
      const stored = localStorage.getItem(storageKey(id));
      return stored === null ? defaultOpen : stored === "1";
    } catch {
      return defaultOpen;
    }
  });

  const toggle = () => {
    const next = !open;
    setOpen(next);
    try {
      localStorage.setItem(storageKey(id), next ? "1" : "0");
    } catch {
      // localStorage unavailable — the state just won't persist.
    }
  };

  return (
    <section className="section">
      <button
        type="button"
        className="section__header"
        onClick={toggle}
        aria-expanded={open}
      >
        <svg
          className={`section__chevron${open ? " section__chevron--open" : ""}`}
          width="9"
          height="9"
          viewBox="0 0 10 10"
          aria-hidden="true"
        >
          <path
            d="M3 1.5 L7 5 L3 8.5"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
        {title}
      </button>
      {open && <div className="section__body">{children}</div>}
    </section>
  );
}
