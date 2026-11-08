import React from "react";
import ReactDOM from "react-dom/client";
import Dock from "./Dock";
import "../shared/base.css";
import "./dock.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Dock />
  </React.StrictMode>,
);
