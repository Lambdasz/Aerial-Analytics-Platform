import "./App.css";
import { PluginManager } from "./component_plugin_manager";

function App() {
  if (window.location.pathname === "/plugins") {
    return <PluginManager />;
  }

  return <main className="container" />;
}

export default App;
