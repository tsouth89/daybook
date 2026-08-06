import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import Capture from "./Capture";
import "./styles.css";

// Both windows load the same bundle; the hash decides which one this is.
const isCapture = window.location.hash === "#capture";

if (isCapture) document.body.classList.add("is-capture");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{isCapture ? <Capture /> : <App />}</React.StrictMode>
);
