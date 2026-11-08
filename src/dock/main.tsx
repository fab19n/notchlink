import React from "react";
import ReactDOM from "react-dom/client";
import Dock from "./Dock";
import "../shared/base.css";
import "./dock.css";

// No browser right-click menu; the dock has its own.
document.addEventListener("contextmenu", (e) => e.preventDefault());

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Dock />
  </React.StrictMode>,
);
