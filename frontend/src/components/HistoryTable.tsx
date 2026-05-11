/**
 * HistoryTable — per-truck history browser.
 *
 * Level 1: Fleet list (up to 10 trucks per page), shows record count and last-seen.
 *          Click a truck → Level 2.
 *
 * Level 2: Full telemetry table for that truck, paginated at 100 records per page
 *          (backed by the /api/history/page endpoint). Columns include timestamp,
 *          state, speed, RPM, elevation, fuel, payload.
 */

import { useState, useEffect, useCallback } from "react";
import { X, ChevronLeft, ChevronRight, ArrowLeft } from "lucide-react";
import type { Fleet, HistoryPage } from "../types";

const API = import.meta.env.VITE_API_BASE_URL ?? "";
const TRUCKS_PER_PAGE = 10;
const RECORDS_PER_PAGE = 100;

// ── Level 1 helpers ───────────────────────────────────────────────────────────

interface TruckSummary {
  fleet: Fleet;
  record_count: number;
  last_seen: string | null;
}

async function fetchFleets(): Promise<Fleet[]> {
  const res = await fetch(`${API}/api/fleets`);
  if (!res.ok) throw new Error("Failed to fetch fleets");
  return res.json();
}

// We derive record_count by fetching page 0 with page_size=1 and reading total_count.
async function fetchTruckSummaries(fleets: Fleet[]): Promise<TruckSummary[]> {
  return Promise.all(
    fleets.map(async (fleet) => {
      try {
        const res = await fetch(
          `${API}/api/history/page?truck_id=${fleet.call_sign}&page=0&page_size=1`
        );
        if (!res.ok) return { fleet, record_count: 0, last_seen: null };
        const page: HistoryPage = await res.json();
        const last_seen = page.points[0]?.timestamp ?? null;
        return { fleet, record_count: page.total_count, last_seen };
      } catch {
        return { fleet, record_count: 0, last_seen: null };
      }
    })
  );
}

// ── Level 2 helpers ───────────────────────────────────────────────────────────

async function fetchHistoryPage(truckId: string, page: number): Promise<HistoryPage> {
  const res = await fetch(
    `${API}/api/history/page?truck_id=${truckId}&page=${page}&page_size=${RECORDS_PER_PAGE}`
  );
  if (!res.ok) throw new Error("Failed to fetch history page");
  return res.json();
}

// ── Sub-components ────────────────────────────────────────────────────────────

function Pagination({
  page,
  totalPages,
  onPrev,
  onNext,
}: {
  page: number;
  totalPages: number;
  onPrev: () => void;
  onNext: () => void;
}) {
  return (
    <div className="flex items-center gap-3 text-xs text-fleet-muted">
      <button
        onClick={onPrev}
        disabled={page === 0}
        className="p-1 rounded hover:bg-fleet-border disabled:opacity-30"
      >
        <ChevronLeft size={14} />
      </button>
      <span>
        Page {page + 1} / {Math.max(1, totalPages)}
      </span>
      <button
        onClick={onNext}
        disabled={page >= totalPages - 1}
        className="p-1 rounded hover:bg-fleet-border disabled:opacity-30"
      >
        <ChevronRight size={14} />
      </button>
    </div>
  );
}

// ── Fleet list (level 1) ──────────────────────────────────────────────────────

interface FleetListProps {
  summaries: TruckSummary[];
  page: number;
  onPageChange: (p: number) => void;
  onSelectTruck: (id: string) => void;
}

function FleetList({ summaries, page, onPageChange, onSelectTruck }: FleetListProps) {
  const totalPages = Math.ceil(summaries.length / TRUCKS_PER_PAGE);
  const slice = summaries.slice(page * TRUCKS_PER_PAGE, (page + 1) * TRUCKS_PER_PAGE);

  return (
    <div className="flex flex-col h-full">
      <div className="flex-1 overflow-auto">
        <table className="w-full text-xs border-collapse">
          <thead>
            <tr className="text-fleet-muted uppercase text-[10px] border-b border-fleet-border">
              <th className="text-left py-2 px-3 font-semibold">Truck</th>
              <th className="text-left py-2 px-3 font-semibold">Type</th>
              <th className="text-left py-2 px-3 font-semibold">Status</th>
              <th className="text-right py-2 px-3 font-semibold">Records</th>
              <th className="text-right py-2 px-3 font-semibold">Last Seen</th>
            </tr>
          </thead>
          <tbody>
            {slice.map(({ fleet, record_count, last_seen }) => (
              <tr
                key={fleet.id}
                onClick={() => onSelectTruck(fleet.call_sign)}
                className="border-b border-fleet-border hover:bg-fleet-border/40 cursor-pointer transition-colors"
              >
                <td className="py-2 px-3 font-mono font-bold text-fleet-text">
                  {fleet.call_sign}
                </td>
                <td className="py-2 px-3 text-fleet-muted">{fleet.fleet_type}</td>
                <td className="py-2 px-3">
                  <span
                    className={`text-[10px] font-bold px-1.5 py-0.5 rounded ${
                      fleet.status === "ACTIVE"
                        ? "bg-fleet-green/20 text-fleet-green"
                        : fleet.status === "BREAKDOWN"
                        ? "bg-fleet-red/20 text-fleet-red"
                        : "bg-fleet-yellow/20 text-fleet-yellow"
                    }`}
                  >
                    {fleet.status}
                  </span>
                </td>
                <td className="py-2 px-3 text-right font-mono">
                  {record_count.toLocaleString()}
                </td>
                <td className="py-2 px-3 text-right text-fleet-muted">
                  {last_seen ? new Date(last_seen).toLocaleString() : "—"}
                </td>
              </tr>
            ))}
            {slice.length === 0 && (
              <tr>
                <td colSpan={5} className="py-6 text-center text-fleet-muted">
                  No trucks found.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
      <div className="flex justify-between items-center px-3 py-2 border-t border-fleet-border">
        <span className="text-[10px] text-fleet-muted">{summaries.length} trucks total</span>
        <Pagination
          page={page}
          totalPages={totalPages}
          onPrev={() => onPageChange(page - 1)}
          onNext={() => onPageChange(page + 1)}
        />
      </div>
    </div>
  );
}

// ── Truck detail table (level 2) ──────────────────────────────────────────────

interface TruckDetailProps {
  truckId: string;
  onBack: () => void;
}

function TruckDetail({ truckId, onBack }: TruckDetailProps) {
  const [histPage, setHistPage] = useState<HistoryPage | null>(null);
  const [currentPage, setCurrentPage] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async (page: number) => {
    setLoading(true);
    setError(null);
    try {
      const data = await fetchHistoryPage(truckId, page);
      setHistPage(data);
      setCurrentPage(page);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [truckId]);

  useEffect(() => {
    load(0);
  }, [load]);

  const totalPages = histPage
    ? Math.ceil(histPage.total_count / RECORDS_PER_PAGE)
    : 1;

  return (
    <div className="flex flex-col h-full">
      {/* Back header */}
      <div className="flex items-center gap-2 px-3 py-2 border-b border-fleet-border">
        <button
          onClick={onBack}
          className="flex items-center gap-1 text-fleet-muted hover:text-fleet-text text-xs"
        >
          <ArrowLeft size={14} /> Fleet List
        </button>
        <span className="text-xs font-bold text-fleet-text ml-2">{truckId}</span>
        {histPage && (
          <span className="text-[10px] text-fleet-muted ml-auto">
            {histPage.total_count.toLocaleString()} records
          </span>
        )}
      </div>

      {/* Table */}
      <div className="flex-1 overflow-auto">
        {loading && (
          <p className="text-center text-fleet-muted text-xs py-8">Loading…</p>
        )}
        {error && (
          <p className="text-center text-fleet-red text-xs py-8">{error}</p>
        )}
        {!loading && !error && histPage && (
          <table className="w-full text-xs border-collapse">
            <thead>
              <tr className="text-fleet-muted uppercase text-[10px] border-b border-fleet-border sticky top-0 bg-fleet-panel">
                <th className="text-left py-2 px-2 font-semibold">Timestamp</th>
                <th className="text-left py-2 px-2 font-semibold">State</th>
                <th className="text-right py-2 px-2 font-semibold">Speed</th>
                <th className="text-right py-2 px-2 font-semibold">RPM</th>
                <th className="text-right py-2 px-2 font-semibold">Elev (m)</th>
                <th className="text-right py-2 px-2 font-semibold">Fuel %</th>
                <th className="text-right py-2 px-2 font-semibold">Payload (t)</th>
              </tr>
            </thead>
            <tbody>
              {histPage.points.map((pt, i) => (
                <tr
                  key={i}
                  className="border-b border-fleet-border/50 hover:bg-fleet-border/20"
                >
                  <td className="py-1.5 px-2 font-mono text-[10px] text-fleet-muted whitespace-nowrap">
                    {new Date(pt.timestamp).toLocaleString()}
                  </td>
                  <td className="py-1.5 px-2">
                    <span
                      className={`text-[9px] font-bold px-1 py-0.5 rounded ${
                        pt.operational_state === "HAULING"
                          ? "bg-blue-500/20 text-blue-400"
                          : pt.operational_state === "LOADING"
                          ? "bg-green-500/20 text-green-400"
                          : pt.operational_state === "DUMPING"
                          ? "bg-orange-500/20 text-orange-400"
                          : pt.operational_state === "RETURNING"
                          ? "bg-purple-500/20 text-purple-400"
                          : "bg-fleet-border text-fleet-muted"
                      }`}
                    >
                      {pt.operational_state ?? "—"}
                    </span>
                  </td>
                  <td className="py-1.5 px-2 text-right font-mono">
                    {pt.speed_kmh != null ? `${pt.speed_kmh.toFixed(1)}` : "—"}
                  </td>
                  <td className="py-1.5 px-2 text-right font-mono">
                    {pt.engine_rpm ?? "—"}
                  </td>
                  <td className="py-1.5 px-2 text-right font-mono">
                    {pt.elevation_meters != null ? pt.elevation_meters.toFixed(0) : "—"}
                  </td>
                  <td className="py-1.5 px-2 text-right font-mono">
                    {pt.fuel_level_percent != null ? pt.fuel_level_percent.toFixed(1) : "—"}
                  </td>
                  <td className="py-1.5 px-2 text-right font-mono">
                    {pt.payload_weight_tons != null ? pt.payload_weight_tons.toFixed(0) : "—"}
                  </td>
                </tr>
              ))}
              {histPage.points.length === 0 && (
                <tr>
                  <td colSpan={7} className="py-6 text-center text-fleet-muted">
                    No records found.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        )}
      </div>

      {/* Pagination footer */}
      <div className="flex justify-between items-center px-3 py-2 border-t border-fleet-border">
        <span className="text-[10px] text-fleet-muted">
          Showing {histPage ? (currentPage * RECORDS_PER_PAGE + 1).toLocaleString() : "—"}–
          {histPage
            ? Math.min(
                (currentPage + 1) * RECORDS_PER_PAGE,
                histPage.total_count
              ).toLocaleString()
            : "—"}{" "}
          of {histPage?.total_count.toLocaleString() ?? "—"}
        </span>
        <Pagination
          page={currentPage}
          totalPages={totalPages}
          onPrev={() => load(currentPage - 1)}
          onNext={() => load(currentPage + 1)}
        />
      </div>
    </div>
  );
}

// ── Main HistoryTable ─────────────────────────────────────────────────────────

interface HistoryTableProps {
  onClose: () => void;
}

export default function HistoryTable({ onClose }: HistoryTableProps) {
  const [summaries, setSummaries] = useState<TruckSummary[]>([]);
  const [fleetPage, setFleetPage] = useState(0);
  const [selectedTruck, setSelectedTruck] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    (async () => {
      setLoading(true);
      try {
        const fleets = await fetchFleets();
        const s = await fetchTruckSummaries(fleets);
        setSummaries(s);
      } finally {
        setLoading(false);
      }
    })();
  }, []);

  return (
    <div className="absolute inset-0 z-[2000] bg-fleet-bg/95 backdrop-blur flex flex-col">
      {/* Modal header */}
      <div className="flex items-center justify-between px-5 py-3 border-b border-fleet-border bg-fleet-panel">
        <h2 className="text-sm font-bold tracking-widest uppercase text-fleet-text">
          Fleet History
        </h2>
        <button
          onClick={onClose}
          className="flex items-center gap-1 text-fleet-muted hover:text-fleet-text text-xs"
        >
          <X size={16} /> Close
        </button>
      </div>

      {/* Body */}
      <div className="flex-1 overflow-hidden">
        {loading ? (
          <p className="text-center text-fleet-muted text-xs py-12">Loading fleet data…</p>
        ) : selectedTruck ? (
          <TruckDetail
            truckId={selectedTruck}
            onBack={() => setSelectedTruck(null)}
          />
        ) : (
          <FleetList
            summaries={summaries}
            page={fleetPage}
            onPageChange={setFleetPage}
            onSelectTruck={setSelectedTruck}
          />
        )}
      </div>
    </div>
  );
}
