import { useState } from "react";
import FleetMap from "./components/FleetMap";
import Sidebar from "./components/Sidebar";
import DrillDown from "./components/DrillDown";
import HistoryScrubber from "./components/HistoryScrubber";
import HistoryTable from "./components/HistoryTable";
import { useSseStream } from "./hooks/useSseStream";

export default function App() {
  useSseStream();
  const [showHistoryTable, setShowHistoryTable] = useState(false);

  return (
    <div className="flex h-screen overflow-hidden bg-fleet-bg">
      <Sidebar onOpenHistoryTable={() => setShowHistoryTable(true)} />

      <div className="relative flex-1">
        <FleetMap />
        <DrillDown />
        <HistoryScrubber />
        {showHistoryTable && (
          <HistoryTable onClose={() => setShowHistoryTable(false)} />
        )}
      </div>
    </div>
  );
}

