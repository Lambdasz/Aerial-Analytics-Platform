import { useEffect, useRef } from "react";
import { FaCaretDown, FaMagnifyingGlass, FaPlaneUp, FaPlus } from "react-icons/fa6";

import "./style.css";

export interface NavbarProps {
  readonly onSearch?: (query: string) => void;
  readonly onAdd?: () => void;
}

export function Navbar({ onSearch, onAdd }: NavbarProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    const handleKey = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement;
      const typing =
        target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable;
      if (event.key === "/" && !typing) {
        event.preventDefault();
        inputRef.current?.focus();
      }
    };
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, []);

  return (
    <header className="app-shell_navbar">
      <div className="app-shell_navbar-left">
        <span className="app-shell_logo">
          <FaPlaneUp aria-hidden="true" />
        </span>

        <div className="app-shell_navbar-title">
          <span className="app-shell_navbar-title-top">Aerial Analytic</span>
          <span className="app-shell_navbar-title-bottom" aria-label="Platforms">
            {"Platforms".split("").map((char, i) => (
              <span key={i} aria-hidden="true">
                {char}
              </span>
            ))}
          </span>
        </div>
      </div>

      <div className="app-shell_navbar-right">
        <label className="app-shell_search">
          <FaMagnifyingGlass aria-hidden="true" />
          <input
            ref={inputRef}
            type="text"
            placeholder="Type / to search"
            onChange={(e) => onSearch?.(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape") inputRef.current?.blur();
            }}
          />
          <kbd className="app-shell_kbd">/</kbd>
        </label>

        <button type="button" className="app-shell_add" aria-label="Add new" onClick={onAdd}>
          <FaPlus aria-hidden="true" />
          <FaCaretDown aria-hidden="true" className="app-shell_add-caret" />
        </button>
      </div>
    </header>
  );
}
