# Role: Senior Full Stack / Frontend Engineer (3D Geospatial & Real-time UI)

## Task
Build a high-performance **Mine Fleet Live Tracker** Single Page Application (SPA). Design a fixed-height, overflow-hidden "Command Center" dashboard with a dark-themed, industrial aesthetic.

## Tech Stack
React, TypeScript, Vite, Tailwind CSS, **CesiumJS (Cesium 3D)**, Lucide-React (Icons), and Zustand (State Management).

## Layout Architecture
- **Main Viewport:** Full-screen Cesium 3D Map (rendering 3D terrain and vehicle models/markers).
- **Left Sidebar:** Fleet Summary & Real-time Alert Management.
- **Right Drawer:** Contextual Vehicle Drill-Down (slides in on marker click).
- **Bottom Overlay:** History Time Scrubber (visible only in History Mode).

## Core Modes & Components

### 1. Live Mode (Real-Time Tracking)
- **Data Source:** Consumes telemetry via `/api/stream` (SSE).
- **Marker Logic & Performance:** 
  - Render and move fleet markers on the 3D terrain.
  - Implement smooth interpolation so 3D markers don't jump between SSE updates.
  - **CRITICAL:** The marker MUST continue moving even if an anomaly is detected. 
  - **Stale Data:** Gray out marker if no data received for >30 seconds.
- **Health Classification (Marker Colors):**
  - `Green`: Normal.
  - `Red`: Critical (Speed > 60km/h OR Engine > 2500 RPM). Changes marker color instantly.

### 2. Alert Management System (Left Sidebar)
- **Live Alert Feed:** Receives live anomaly events from the backend via SSE.
- **Acknowledgement Flow:** 
  - Unhandled alerts appear in an "Unacknowledged" queue.
  - Provide an "Acknowledge" button for each alert. Clicking it triggers a `PUT /api/alerts/{id}/acknowledge` request to the backend.
  - Once acknowledged (successful API response), move the alert to the "Alert History" section and reset the vehicle marker to Green (assuming the telemetry is back to normal).

### 3. History Mode (Audit View)
- **State Logic:** Entering this mode pauses live SSE updates.
- **Data Source:** Fetches array of historical coordinates and alerts from `/api/history?truck_id=XXX`.
- **Visuals:** 
  - Render a static 3D Polyline representing the full traveled route on the Cesium map.
  - Display an Alert History Data Table.
- **Time Scrubber:** Moving the slider moves the truck marker along the 3D Polyline and syncs the telemetry widget to that specific timestamp.

### 4. Vehicle Drill-Down (Right Drawer)
- Displays a live telemetry grid (Speed, RPM, Load, Fuel %). 
- Renders a 15-minute "breadcrumb" recent trail (Polyline) behind the selected truck to show immediate past movement.

---
**Action Request:** 
Please generate the boilerplate for the `CesiumJS` Map Engine integration first (handling the initialization of the 3D viewer and camera). Then, create the Zustand store for handling the live SSE telemetry stream and the Alert Acknowledgement logic.