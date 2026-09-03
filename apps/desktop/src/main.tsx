import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import EdgeOverlay from "./EdgeOverlay";

const params = new URLSearchParams(window.location.search);
const isEdgeSurface = params.get("surface") === "edge";

createRoot(document.getElementById("root")!).render(
  <StrictMode>{isEdgeSurface ? <EdgeOverlay /> : <App />}</StrictMode>,
);
