"""Sensor state change logging for Loxone MCP.

Logs all sensor state changes with timestamps for analysis and monitoring.
Provides historical data and change tracking for door/window sensors.

SPDX-License-Identifier: MIT
Copyright (c) 2025 Ralf Anton Beier
"""

import asyncio
import contextlib
import json
import logging
import time
from collections import deque
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

logger = logging.getLogger(__name__)


@dataclass
class StateChangeEvent:
    """A recorded sensor state change event."""

    uuid: str
    timestamp: float
    old_value: Any
    new_value: Any
    human_readable: str  # "OPEN", "CLOSED", etc.
    event_type: str = "state_change"


@dataclass
class SensorStateHistory:
    """Complete state history for a sensor with ring buffer."""

    uuid: str
    first_seen: float
    last_updated: float
    total_changes: int
    current_state: Any
    state_events: deque  # Ring buffer for events
    max_events: int = 100  # Maximum events to keep per sensor


class SensorStateLogger:
    """In-memory sensor state logger with periodic persistence and ring buffers."""

    def __init__(
        self,
        log_file: Path | None = None,
        max_events_per_sensor: int = 100,
        max_sensors: int = 1000,
        sync_interval: int = 600,
    ) -> None:
        """
        Initialize in-memory state logger with ring buffers.

        Args:
            log_file: Optional file to persist logs to
            max_events_per_sensor: Maximum events to keep per sensor (ring buffer size)
            max_sensors: Maximum number of sensors to track
            sync_interval: Seconds between automatic syncs (default: 10 minutes)
        """
        self.log_file = log_file or Path("sensor_state_log.json")
        self.max_events_per_sensor = max_events_per_sensor
        self.max_sensors = max_sensors
        self.sync_interval = sync_interval

        # In-memory state tracking with ring buffers
        self.sensor_histories: dict[str, SensorStateHistory] = {}
        self.session_start = time.time()
        self.last_sync = time.time()
        self.pending_changes = 0

        # Sync task
        self._sync_task: asyncio.Task | None = None
        self._shutdown_requested = False

        # Load existing logs if file exists
        self._load_existing_logs()

        # Start periodic sync task
        self._start_sync_task()

    def _start_sync_task(self) -> None:
        """Start the periodic sync task."""
        try:
            loop = asyncio.get_event_loop()
            self._sync_task = loop.create_task(self._periodic_sync())
        except RuntimeError:
            # No event loop running, sync will happen manually
            logger.debug("No event loop available for periodic sync")

    async def _periodic_sync(self) -> None:
        """Periodically sync data to disk."""
        while not self._shutdown_requested:
            try:
                await asyncio.sleep(self.sync_interval)
                if self.pending_changes > 0:
                    self.persist_logs()
                    logger.debug(f"Periodic sync completed ({self.pending_changes} changes)")
                    self.pending_changes = 0
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Error in periodic sync: {e}")

    def _load_existing_logs(self) -> None:
        """Load existing log data from file with ring buffer support."""
        if not self.log_file.exists():
            logger.info(f"No existing log file found at {self.log_file}")
            return

        try:
            with open(self.log_file) as f:
                data = json.load(f)

            loaded_count = 0
            for uuid, history_data in data.get("sensor_histories", {}).items():
                # Only load up to max_sensors
                if loaded_count >= self.max_sensors:
                    logger.warning(
                        f"Reached max sensors limit ({self.max_sensors}), skipping remaining"
                    )
                    break

                # Reconstruct state events as ring buffer
                events = deque(maxlen=self.max_events_per_sensor)
                events_data = history_data.get("state_events", [])[-self.max_events_per_sensor :]
                for event_data in events_data:
                    events.append(StateChangeEvent(**event_data))

                # Reconstruct sensor history with ring buffer
                history = SensorStateHistory(
                    uuid=history_data["uuid"],
                    first_seen=history_data["first_seen"],
                    last_updated=history_data["last_updated"],
                    total_changes=history_data["total_changes"],
                    current_state=history_data["current_state"],
                    state_events=events,
                    max_events=self.max_events_per_sensor,
                )
                self.sensor_histories[uuid] = history
                loaded_count += 1

            logger.info(f"Loaded state history for {loaded_count} sensors from {self.log_file}")

        except Exception as e:
            logger.warning(f"Failed to load existing logs: {e}")

    def log_state_change(self, uuid: str, old_value: Any, new_value: Any) -> None:
        """
        Log a sensor state change in memory with ring buffer.

        Args:
            uuid: Sensor UUID
            old_value: Previous value
            new_value: New value
        """
        current_time = time.time()

        # Check if we're at sensor limit
        if uuid not in self.sensor_histories and len(self.sensor_histories) >= self.max_sensors:
            # Remove oldest sensor if at limit
            oldest_uuid = min(
                self.sensor_histories.keys(), key=lambda u: self.sensor_histories[u].last_updated
            )
            del self.sensor_histories[oldest_uuid]
            logger.debug(f"Removed oldest sensor {oldest_uuid} to make room for {uuid}")

        # Determine human-readable state
        human_readable = self._get_human_readable_state(new_value)

        # Create event
        event = StateChangeEvent(
            uuid=uuid,
            timestamp=current_time,
            old_value=old_value,
            new_value=new_value,
            human_readable=human_readable,
        )

        # Update or create sensor history with ring buffer
        if uuid in self.sensor_histories:
            history = self.sensor_histories[uuid]
            history.last_updated = current_time
            history.total_changes += 1
            history.current_state = new_value
            history.state_events.append(event)  # Ring buffer auto-handles size limit

        else:
            # New sensor with ring buffer
            events = deque(maxlen=self.max_events_per_sensor)
            events.append(event)

            history = SensorStateHistory(
                uuid=uuid,
                first_seen=current_time,
                last_updated=current_time,
                total_changes=1,
                current_state=new_value,
                state_events=events,
                max_events=self.max_events_per_sensor,
            )
            self.sensor_histories[uuid] = history

        # Track pending changes for sync
        self.pending_changes += 1

        # Log the change (only for important events to avoid spam)
        if human_readable in ["OPEN", "CLOSED"]:
            logger.info(f"Door/Window: {uuid[-8:]} {old_value} → {new_value} ({human_readable})")
        else:
            logger.debug(f"Sensor: {uuid[-8:]} {old_value} → {new_value} ({human_readable})")

    def _get_human_readable_state(self, value: Any) -> str:
        """Convert numeric state to human-readable format."""
        if value == 0 or value == 0.0:
            return "OPEN"
        elif value == 1 or value == 1.0:
            return "CLOSED"
        elif isinstance(value, str):
            return value.upper()
        else:
            return f"VALUE({value})"

    async def shutdown(self) -> None:
        """Gracefully shutdown the logger and persist final state."""
        logger.info("Shutting down sensor state logger...")

        self._shutdown_requested = True

        # Cancel sync task
        if self._sync_task and not self._sync_task.done():
            self._sync_task.cancel()
            with contextlib.suppress(asyncio.CancelledError):
                await self._sync_task

        # Final sync
        if self.pending_changes > 0:
            self.persist_logs()
            logger.info(f"Final sync completed ({self.pending_changes} changes)")

        logger.info("Sensor state logger shutdown complete")

    def persist_logs(self) -> None:
        """Persist all logs to file."""
        try:
            # Convert to serializable format
            data = {
                "session_start": self.session_start,
                "last_persisted": time.time(),
                "sensor_histories": {},
            }

            for uuid, history in self.sensor_histories.items():
                data["sensor_histories"][uuid] = {
                    "uuid": history.uuid,
                    "first_seen": history.first_seen,
                    "last_updated": history.last_updated,
                    "total_changes": history.total_changes,
                    "current_state": history.current_state,
                    "state_events": [asdict(event) for event in list(history.state_events)],
                }

            # Write to file
            with open(self.log_file, "w") as f:
                json.dump(data, f, indent=2)

            logger.debug(f"Persisted state logs to {self.log_file}")

        except Exception as e:
            logger.error(f"Failed to persist logs: {e}")

    def get_sensor_history(self, uuid: str) -> SensorStateHistory | None:
        """Get complete history for a sensor."""
        return self.sensor_histories.get(uuid)

    def get_recent_changes(self, limit: int = 50) -> list[StateChangeEvent]:
        """Get recent state changes across all sensors."""
        all_events = []
        for history in self.sensor_histories.values():
            all_events.extend(list(history.state_events))

        # Sort by timestamp (newest first)
        all_events.sort(key=lambda e: e.timestamp, reverse=True)
        return all_events[:limit]

    def get_changes_since(self, since_timestamp: float) -> list[StateChangeEvent]:
        """Get all state changes since a specific timestamp."""
        changes = []
        for history in self.sensor_histories.values():
            for event in list(history.state_events):
                if event.timestamp >= since_timestamp:
                    changes.append(event)

        # Sort by timestamp
        changes.sort(key=lambda e: e.timestamp)
        return changes

    def get_door_window_activity(self, hours: int = 24) -> dict[str, Any]:
        """Get door/window activity summary for the last N hours."""
        since_timestamp = time.time() - (hours * 3600)
        recent_changes = self.get_changes_since(since_timestamp)

        # Filter for door/window changes (OPEN/CLOSED)
        door_window_changes = [e for e in recent_changes if e.human_readable in ["OPEN", "CLOSED"]]

        # Count by sensor
        sensor_activity = {}
        for event in door_window_changes:
            if event.uuid not in sensor_activity:
                sensor_activity[event.uuid] = {
                    "total_changes": 0,
                    "opens": 0,
                    "closes": 0,
                    "current_state": None,
                    "last_change": None,
                }

            activity = sensor_activity[event.uuid]
            activity["total_changes"] += 1
            if event.human_readable == "OPEN":
                activity["opens"] += 1
            else:
                activity["closes"] += 1
            activity["current_state"] = event.human_readable
            activity["last_change"] = event.timestamp

        return {
            "period_hours": hours,
            "total_changes": len(door_window_changes),
            "sensors_active": len(sensor_activity),
            "sensor_activity": sensor_activity,
            "timeline": [
                {
                    "timestamp": e.timestamp,
                    "uuid": e.uuid,
                    "change": f"{e.old_value} → {e.new_value}",
                    "human": e.human_readable,
                }
                for e in door_window_changes[-20:]  # Last 20 changes
            ],
        }

    def get_statistics(self) -> dict[str, Any]:
        """Get overall logging statistics."""
        if not self.sensor_histories:
            return {"status": "no_data"}

        total_events = sum(len(h.state_events) for h in self.sensor_histories.values())

        return {
            "session_start": self.session_start,
            "sensors_tracked": len(self.sensor_histories),
            "total_events": total_events,
            "log_file": str(self.log_file),
            "oldest_event": min(h.first_seen for h in self.sensor_histories.values()),
            "newest_event": max(h.last_updated for h in self.sensor_histories.values()),
            "most_active_sensor": max(
                self.sensor_histories.items(), key=lambda x: x[1].total_changes
            )[0]
            if self.sensor_histories
            else None,
        }


# Global state logger instance
_state_logger: SensorStateLogger | None = None


def get_state_logger() -> SensorStateLogger:
    """Get the global state logger instance."""
    global _state_logger
    if _state_logger is None:
        _state_logger = SensorStateLogger()
    return _state_logger


def init_state_logger(log_file: Path | None = None) -> SensorStateLogger:
    """Initialize the global state logger."""
    global _state_logger
    _state_logger = SensorStateLogger(log_file)
    return _state_logger
