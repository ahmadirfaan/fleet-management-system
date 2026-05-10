import FleetMap from "./components/FleetMap";
import Sidebar from "./components/Sidebar";
import DrillDown from "./components/DrillDown";
import HistoryScrubber from "./components/HistoryScrubber";
import { useSseStream } from "./hooks/useSseStream";

export default function App() {
  // Start SSE stream and alert polling.
  useSseStream();

  return (
    <div className="flex h-screen overflow-hidden bg-fleet-bg">
      {/* Left sidebar */}
      <Sidebar />

      {/* Main map area with overlays */}
      <div className="relative flex-1">
        <FleetMap />
        <DrillDown />
        <HistoryScrubber />
      </div>
    </div>
  );
}
