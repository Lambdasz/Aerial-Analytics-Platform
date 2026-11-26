import { NewMenu } from "./NewMenu";

import "./style.css";

export function Navbar() {
  return (
    <header className="app-shell_navbar">
      <div className="app-shell_navbar-brand">
        <span className="app-shell_navbar-bold">Aerial</span>
        <span>Analysis</span>
      </div>

      <div className="app-shell_navbar-end">
        <NewMenu />
      </div>
    </header>
  );
}
