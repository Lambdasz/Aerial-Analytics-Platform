import { useEffect, useRef } from "react";
import { FaBars, FaCaretDown, FaMagnifyingGlass, FaPlaneUp, FaPlus } from "react-icons/fa6";

import "./style.css";

export interface NavbarProps {
  readonly sidebarOpen: boolean;
  readonly onToggleSidebar: () => void;
  readonly onSearch?: (query: string) => void;
  readonly onAdd?: () => void;
}

export function Navbar({ sidebarOpen, onToggleSidebar, onSearch, onAdd }: NavbarProps) {
  const inputRef = useRef<HTMLInputElement>(null);

  // Press "/" anywhere to focus search, like GitHub
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
      {/* Left: burger, search, add */}
      <div className="app-shell_navbar-left">
        <button
          type="button"
          className="app-shell_burger"
          aria-label={sidebarOpen ? "Close sidebar" : "Open sidebar"}
          aria-expanded={sidebarOpen}
          onClick={onToggleSidebar}
        >
          <FaBars aria-hidden="true" />
        </button>

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

      {/* Right: program name + logo */}
      <div className="app-shell_navbar-right">
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

        <span className="app-shell_logo">
          <FaPlaneUp aria-hidden="true" />
        </span>
      </div>
    </header>
  );
}
