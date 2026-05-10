import { useEffect, useRef } from "react";
import { useFleetStore } from "../store/fleetStore";
import type { TelemetryEvent, Alert } from "../types";

const API = import.meta.env.VITE_API_BASE_URL ?? "";

/**
 * Opens an SSE connection to /api/stream and feeds events into the Zustand store.
 * Also polls /api/alerts every 10 seconds for the alert sidebar.
 */
export function useSseStream() {
  const applyTelemetry = useFleetStore((s) => s.applyTelemetry);
  const setAlerts = useFleetStore((s) => s.setAlerts);
  const isLiveMode = useFleetStore((s) => s.isLiveMode);
  const markStale = useFleetStore((s) => s.markStale);

  // SSE connection ref so we can close it when history mode is active.
  const esRef = useRef<EventSource | null>(null);

  useEffect(() => {
    if (!isLiveMode) {
      esRef.current?.close();
      return;
    }

    const es = new EventSource(`${API}/api/stream`);
    esRef.current = es;

    es.onmessage = (e) => {
      try {
        const event: TelemetryEvent = JSON.parse(e.data);
        applyTelemetry(event);
      } catch {
        // Malformed SSE message – ignore.
      }
    };

    es.onerror = () => {
      // Browser will auto-reconnect for SSE.
    };

    return () => {
      es.close();
    };
  }, [isLiveMode, applyTelemetry]);

  // Poll alerts.
  useEffect(() => {
    const fetchAlerts = async () => {
      try {
        const res = await fetch(`${API}/api/alerts`);
        const data: Alert[] = await res.json();
        setAlerts(data);
      } catch {
        // Network error – silently ignore.
      }
    };
    fetchAlerts();
    const interval = setInterval(fetchAlerts, 10_000);
    return () => clearInterval(interval);
  }, [setAlerts]);

  // Stale marker check every 5 seconds.
  useEffect(() => {
    const interval = setInterval(markStale, 5_000);
    return () => clearInterval(interval);
  }, [markStale]);
}
