import React from "react";
import ReactDOM from "react-dom/client";
import Edge from "./Edge";
import "../shared/base.css";
import "./edge.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Edge />
  </React.StrictMode>,
);
