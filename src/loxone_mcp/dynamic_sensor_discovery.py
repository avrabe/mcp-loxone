"""Dynamic sensor discovery from live WebSocket data.

Automatically discovers door/window sensors by analyzing real-time state updates
from the WebSocket connection. No configuration files needed.

SPDX-License-Identifier: MIT
Copyright (c) 2025 Ralf Anton Beier
"""

import asyncio
import logging
import time
from dataclasses import dataclass
from typing import Any

logger = logging.getLogger(__name__)


# Sensor categorization patterns
SENSOR_CATEGORIES = {
    "door_window": {
        "name": "Door/Window Sensors",
        "criteria": {
            "binary_only": True,  # Only 0/1 values
            "max_updates": 3,  # Very stable (max 3 updates during discovery)
            "min_activity": 1,  # Some activity required
            "stable_pattern": True,  # Consistent behavior
            "require_change": True,  # Must have actual state changes
        },
        "priority": 10,
    },
    "motion": {
        "name": "Motion Sensors",
        "criteria": {
            "binary_only": True,
            "max_updates": 100,  # Can be more active
            "min_activity": 5,
            "stable_pattern": False,  # Can be irregular
        },
        "priority": 8,
    },
    "analog": {
        "name": "Analog Sensors",
        "criteria": {
            "binary_only": False,
            "value_range": (0, 1000),  # Reasonable analog range
            "min_activity": 1,
        },
        "priority": 5,
    },
    "noisy": {
        "name": "Noisy/System Sensors",
        "criteria": {
            "max_updates": 1000,  # Very chatty
            "min_activity": 50,
        },
        "priority": 1,
    },
}


@dataclass
class DiscoveredSensor:
    """A dynamically discovered sensor with categorization."""

    uuid: str
    current_value: Any
    value_history: list[Any]
    first_seen: float
    last_updated: float
    update_count: int
    is_binary: bool = False  # True if only sees 0/1 values
    is_door_window: bool = False  # True if likely a door/window sensor
    category: str = "unknown"  # Sensor category
    confidence: float = 0.0  # Confidence score (0-1)
    pattern_score: float = 0.0  # Pattern analysis score


class DynamicSensorDiscovery:
    """Discovers sensors dynamically from WebSocket state updates."""

    def __init__(self, discovery_time: float = 60.0) -> None:
        """
        Initialize dynamic sensor discovery.

        Args:
            discovery_time: How long to monitor for discovery (seconds)
        """
        self.discovery_time = discovery_time
        self.sensors: dict[str, DiscoveredSensor] = {}
        self.discovery_active = False
        self.start_time: float | None = None

        # Pattern matching for door/window sensors
        self.door_window_patterns = [
            "fenster",  # German for window
            "tür",
            "tuer",  # German for door
            "window",
            "door",
            "sensor",
            "kontakt",  # German for contact
        ]

    def start_discovery(self) -> None:
        """Start sensor discovery process."""
        logger.info("Starting dynamic sensor discovery...")
        self.discovery_active = True
        self.start_time = time.time()
        self.sensors.clear()

    def stop_discovery(self) -> None:
        """Stop sensor discovery process."""
        if self.discovery_active:
            logger.info("Stopping dynamic sensor discovery...")
            self.discovery_active = False
            self._analyze_discovered_sensors()

    def on_state_update(self, uuid: str, value: Any) -> None:
        """
        Handle state update from WebSocket.

        Args:
            uuid: Sensor UUID
            value: New sensor value
        """
        if not self.discovery_active:
            return

        current_time = time.time()

        if uuid in self.sensors:
            # Update existing sensor
            sensor = self.sensors[uuid]
            if sensor.current_value != value:
                sensor.value_history.append(value)
                sensor.current_value = value
                sensor.last_updated = current_time
                sensor.update_count += 1
        else:
            # New sensor discovered
            sensor = DiscoveredSensor(
                uuid=uuid,
                current_value=value,
                value_history=[value],
                first_seen=current_time,
                last_updated=current_time,
                update_count=1,
            )
            self.sensors[uuid] = sensor
            logger.debug(f"Discovered new sensor: {uuid} = {value}")

    def _analyze_discovered_sensors(self) -> None:
        """Analyze and categorize discovered sensors with improved filtering."""
        logger.info(f"Analyzing {len(self.sensors)} discovered sensors...")

        categorized_sensors = {category: [] for category in SENSOR_CATEGORIES}
        categorized_sensors["unknown"] = []

        for _uuid, sensor in self.sensors.items():
            unique_values = set(sensor.value_history)

            # Categorize each sensor
            best_category = "unknown"
            best_score = 0.0

            for category, config in SENSOR_CATEGORIES.items():
                score = self._calculate_category_score(sensor, unique_values, config["criteria"])
                if score > best_score:
                    best_score = score
                    best_category = category

            # Apply category
            sensor.category = best_category
            sensor.confidence = best_score
            sensor.pattern_score = self._calculate_pattern_score(sensor, unique_values)

            # Set legacy flags for compatibility
            if best_category == "door_window":
                sensor.is_door_window = True
                sensor.is_binary = True
            elif unique_values.issubset({0, 1}):
                sensor.is_binary = True

            categorized_sensors[best_category].append(sensor)

        # Log categorization results
        logger.info("Sensor categorization complete:")
        for category, sensors in categorized_sensors.items():
            if sensors:
                config = SENSOR_CATEGORIES.get(category, {"name": "Unknown"})
                logger.info(f"  {config.get('name', category)}: {len(sensors)} sensors")

        # Show top door/window sensors
        door_window_sensors = sorted(
            categorized_sensors["door_window"], key=lambda s: s.confidence, reverse=True
        )

        if door_window_sensors:
            logger.info("Top door/window sensors (showing up to 15):")
            for i, sensor in enumerate(door_window_sensors[:15]):
                current_state = "OPEN" if sensor.current_value == 0 else "CLOSED"
                logger.info(
                    f"  {i + 1:2d}. {sensor.uuid}: {current_state} "
                    f"(confidence: {sensor.confidence:.2f}, updates: {sensor.update_count})"
                )
        else:
            logger.warning("No door/window sensors found during discovery period")
            logger.info("Try opening/closing doors or windows during discovery")

    def _calculate_category_score(
        self, sensor: DiscoveredSensor, unique_values: set, criteria: dict
    ) -> float:
        """Calculate how well a sensor matches a category's criteria."""
        score = 0.0
        max_score = 0.0

        # Binary only check - STRICT: only exact 0/1 values
        if "binary_only" in criteria:
            max_score += 30
            if criteria["binary_only"]:
                # Only allow exact binary values (0, 1) - no floats
                strict_binary = unique_values.issubset({0, 1})
                if strict_binary:
                    score += 30
                else:
                    # Immediate disqualification for door/window if not strictly binary
                    return 0.0
            else:
                if not unique_values.issubset({0, 1}):
                    score += 30

        # Require actual state changes for door/window sensors FIRST
        if criteria.get("require_change"):
            max_score += 25
            if len(unique_values) >= 2:  # Must have at least OPEN and CLOSED states
                score += 25
            else:
                # Immediate disqualification if no state changes
                return 0.0

        # Update count checks
        if "max_updates" in criteria:
            max_score += 20
            if sensor.update_count <= criteria["max_updates"]:
                score += 20
            else:
                # Steep penalty for being too chatty
                excess = sensor.update_count - criteria["max_updates"]
                score += max(0, 20 - excess * 10)  # Steeper penalty

        if "min_activity" in criteria:
            max_score += 15
            if sensor.update_count >= criteria["min_activity"]:
                score += 15

        # Value range check for analog sensors
        if "value_range" in criteria:
            max_score += 10
            min_val, max_val = criteria["value_range"]
            if all(isinstance(v, int | float) and min_val <= v <= max_val for v in unique_values):
                score += 10

        # Stable pattern check - VERY strict for door/window
        if "stable_pattern" in criteria:
            max_score += 10
            if criteria["stable_pattern"]:
                # Perfect door/window: exactly 2 states (0,1) and very few updates
                if len(unique_values) == 2 and unique_values == {0, 1} and sensor.update_count <= 3:
                    score += 10
                elif len(unique_values) == 2 and sensor.update_count <= 3:
                    score += 5  # Partial credit
                else:
                    # Penalty for not having perfect pattern
                    score += 0
            else:
                # Unstable sensors can have more varied patterns
                score += 8

        return score / max_score if max_score > 0 else 0.0

    def _calculate_pattern_score(self, sensor: DiscoveredSensor, unique_values: set) -> float:
        """Calculate a pattern quality score for the sensor."""
        score = 0.0

        # Perfect binary behavior
        if unique_values.issubset({0, 1}):
            score += 0.4

        # Reasonable activity level
        if 1 <= sensor.update_count <= 20:
            score += 0.3
        elif sensor.update_count <= 50:
            score += 0.2

        # Value consistency
        if len(unique_values) <= 3:
            score += 0.2

        # Recent activity (within last 60 seconds of discovery)
        if time.time() - sensor.last_updated < 60:
            score += 0.1

        return min(score, 1.0)

    def get_door_window_sensors(self) -> list[DiscoveredSensor]:
        """Get all discovered door/window sensors."""
        return [sensor for sensor in self.sensors.values() if sensor.is_door_window]

    def get_binary_sensors(self) -> list[DiscoveredSensor]:
        """Get all discovered binary sensors."""
        return [sensor for sensor in self.sensors.values() if sensor.is_binary]

    def get_all_sensors(self) -> list[DiscoveredSensor]:
        """Get all discovered sensors."""
        return list(self.sensors.values())

    def get_discovery_stats(self) -> dict[str, Any]:
        """Get discovery statistics."""
        if not self.start_time:
            return {"status": "not_started"}

        elapsed = time.time() - self.start_time
        door_window_count = len(self.get_door_window_sensors())
        binary_count = len(self.get_binary_sensors())
        total_count = len(self.sensors)

        return {
            "status": "active" if self.discovery_active else "completed",
            "elapsed_time": elapsed,
            "total_sensors": total_count,
            "binary_sensors": binary_count,
            "door_window_sensors": door_window_count,
            "discovery_time": self.discovery_time,
        }


async def discover_sensors_automatically(
    websocket_client: Any, discovery_time: float = 60.0
) -> list[DiscoveredSensor]:
    """
    Automatically discover door/window sensors from WebSocket data.

    Args:
        websocket_client: Connected WebSocket client
        discovery_time: How long to monitor for discovery

    Returns:
        List of discovered door/window sensors
    """
    logger.info(f"Starting automatic sensor discovery for {discovery_time} seconds...")

    discovery = DynamicSensorDiscovery(discovery_time)

    # Add our discovery callback to the WebSocket client
    websocket_client.add_state_callback(discovery.on_state_update)

    try:
        discovery.start_discovery()

        # Monitor for the specified time
        await asyncio.sleep(discovery_time)

        discovery.stop_discovery()

        # Get results
        door_window_sensors = discovery.get_door_window_sensors()
        stats = discovery.get_discovery_stats()

        logger.info(f"Discovery completed: {stats}")
        logger.info(f"Found {len(door_window_sensors)} door/window sensors")

        return door_window_sensors

    finally:
        # Remove our callback
        websocket_client.remove_state_callback(discovery.on_state_update)
